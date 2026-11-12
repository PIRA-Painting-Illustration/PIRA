//! File › Timelapse › Export Timelapse on Android: the frames go through the system's own H.264
//! encoder (MediaCodec, hardware on most tablets) and into an MP4 written by
//! [`photocraft_codecs::mp4`]. No codec ships with the app.
//!
//! MediaCodec takes YUV 4:2:0 frames; [`rgba_to_yuv420`] converts (BT.601, limited range, the
//! encoder's default) into whichever of the two common layouts the encoder accepted: NV12 (one
//! interleaved UV plane, `COLOR_FormatYUV420SemiPlanar`) or I420 (separate U and V planes,
//! `COLOR_FormatYUV420Planar`), with the row stride and plane height it reports.

/// `MediaCodecInfo.CodecCapabilities` colour formats.
pub const COLOR_SEMI_PLANAR: i32 = 21;
pub const COLOR_PLANAR: i32 = 19;

/// Where an encoder wants the planes of one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub width: usize,
    pub height: usize,
    /// Bytes per luma row.
    pub stride: usize,
    /// Rows in the luma plane (≥ height).
    pub slice_height: usize,
    /// NV12 (`true`) or I420.
    pub semi_planar: bool,
}

impl Layout {
    /// Bytes one frame takes.
    pub fn frame_bytes(&self) -> usize {
        self.stride * self.slice_height * 3 / 2
    }
}

/// `rgba` (`width` × `height`, straight alpha ignored) as YUV 4:2:0 in `out` laid out as
/// `layout` says. `out` must hold [`Layout::frame_bytes`].
pub fn rgba_to_yuv420(rgba: &[u8], l: &Layout, out: &mut [u8]) -> Result<(), String> {
    let (w, h) = (l.width, l.height);
    if rgba.len() < w * h * 4 || out.len() < l.frame_bytes() || l.stride < w || l.slice_height < h || w % 2 != 0 || h % 2 != 0 {
        return Err("a timelapse frame doesn't fit the video encoder's buffer".into());
    }
    let px = |x: usize, y: usize| {
        let i = (y * w + x) * 4;
        (i32::from(rgba[i]), i32::from(rgba[i + 1]), i32::from(rgba[i + 2]))
    };
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = px(x, y);
            out[y * l.stride + x] = (16 + ((66 * r + 129 * g + 25 * b + 128) >> 8)).clamp(0, 255) as u8;
        }
    }
    let chroma = l.stride * l.slice_height;
    let (cw, ch) = (w / 2, h / 2);
    // I420: the U plane, then V, each a quarter of the luma plane with half its stride.
    let quarter = (l.stride / 2) * (l.slice_height / 2);
    for cy in 0..ch {
        for cx in 0..cw {
            let mut sum = (0, 0, 0);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (r, g, b) = px(cx * 2 + dx, cy * 2 + dy);
                sum = (sum.0 + r, sum.1 + g, sum.2 + b);
            }
            let (r, g, b) = ((sum.0 + 2) / 4, (sum.1 + 2) / 4, (sum.2 + 2) / 4);
            let u = (128 + ((-38 * r - 74 * g + 112 * b + 128) >> 8)).clamp(0, 255) as u8;
            let v = (128 + ((112 * r - 94 * g - 18 * b + 128) >> 8)).clamp(0, 255) as u8;
            if l.semi_planar {
                let i = chroma + cy * l.stride + cx * 2;
                out[i] = u;
                out[i + 1] = v;
            } else {
                let i = cy * (l.stride / 2) + cx;
                out[chroma + i] = u;
                out[chroma + quarter + i] = v;
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "android")]
pub use android::factory;

#[cfg(target_os = "android")]
mod android {
    use super::{COLOR_PLANAR, COLOR_SEMI_PLANAR, Layout, rgba_to_yuv420};
    use ndk::media::media_codec::{DequeuedInputBufferResult, DequeuedOutputBufferInfoResult, MediaCodec, MediaCodecDirection};
    use ndk::media::media_format::MediaFormat;
    use photocraft_codecs::mp4::Mp4Writer;
    use photocraft_engine::timelapse_cmds::{VideoEncoder, VideoEncoderFactory};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    const KEY_FRAME: u32 = 1;
    const CODEC_CONFIG: u32 = 2;
    const END_OF_STREAM: u32 = 4;
    const WAIT: Duration = Duration::from_millis(10);
    /// The longest an encoder may take to hand back the end of the stream.
    const DRAIN_LIMIT: Duration = Duration::from_secs(10);

    pub fn factory() -> VideoEncoderFactory {
        Arc::new(|w, h, fps| Encoder::new(w, h, fps).map(|e| Box::new(e) as Box<dyn VideoEncoder>))
    }

    struct Encoder {
        codec: MediaCodec,
        mux: Mp4Writer,
        layout: Layout,
        fps: u64,
        frame: u64,
        yuv: Vec<u8>,
        ended: bool,
    }

    fn err(what: &str, e: impl std::fmt::Debug) -> String {
        format!("the video encoder failed ({what}): {e:?}")
    }

    impl Encoder {
        fn new(w: u32, h: u32, fps: u32) -> Result<Self, String> {
            let mut last = String::from("no H.264 encoder on this device");
            // NV12 first: nearly every hardware encoder takes it; I420 as the fallback.
            for color in [COLOR_SEMI_PLANAR, COLOR_PLANAR] {
                let Some(codec) = MediaCodec::from_encoder_type("video/avc") else { break };
                let mut f = MediaFormat::new();
                f.set_str("mime", "video/avc");
                f.set_i32("width", w as i32);
                f.set_i32("height", h as i32);
                f.set_i32("color-format", color);
                // About 4 bits a pixel a second: sharp line art at a size social sites keep.
                f.set_i32("bitrate", (w * h * 4).clamp(1_000_000, 12_000_000) as i32);
                f.set_i32("frame-rate", fps as i32);
                f.set_i32("i-frame-interval", 1);
                if let Err(e) = codec.configure(&f, None, MediaCodecDirection::Encoder) {
                    last = err("configure", e);
                    continue;
                }
                if let Err(e) = codec.start() {
                    last = err("start", e);
                    continue;
                }
                // The encoder's own input layout needs API 28 to read back; frames are multiples of
                // 16 on both sides, where encoders lay the planes out tightly.
                let layout = Layout { width: w as usize, height: h as usize, stride: w as usize, slice_height: h as usize, semi_planar: color != COLOR_PLANAR };
                log::info!("timelapse: H.264 encoder {w}x{h}, colour format {color}");
                return Ok(Encoder {
                    codec,
                    mux: Mp4Writer::new(w, h, fps),
                    layout,
                    fps: u64::from(fps.max(1)),
                    frame: 0,
                    yuv: vec![0; layout.frame_bytes()],
                    ended: false,
                });
            }
            Err(last)
        }

        /// Takes what the encoder has finished; with `until_end`, waits for the end of stream.
        fn drain(&mut self, until_end: bool) -> Result<(), String> {
            let start = Instant::now();
            loop {
                match self.codec.dequeue_output_buffer(if until_end { WAIT } else { Duration::ZERO }).map_err(|e| err("output", e))? {
                    DequeuedOutputBufferInfoResult::Buffer(buf) => {
                        let info = *buf.info();
                        let (off, size) = (info.offset().max(0) as usize, info.size().max(0) as usize);
                        let data = buf.buffer();
                        if let Some(bytes) = data.get(off..off + size)
                            && size > 0
                        {
                            // A config buffer (SPS/PPS) only sets up the track; the writer tells
                            // them apart itself.
                            self.mux.push_annex_b(bytes, info.flags() & KEY_FRAME != 0 && info.flags() & CODEC_CONFIG == 0);
                        }
                        let eos = info.flags() & END_OF_STREAM != 0;
                        self.codec.release_output_buffer(buf, false).map_err(|e| err("release", e))?;
                        if eos {
                            self.ended = true;
                            return Ok(());
                        }
                    }
                    DequeuedOutputBufferInfoResult::TryAgainLater => {
                        if !until_end {
                            return Ok(());
                        }
                        if start.elapsed() > DRAIN_LIMIT {
                            return Err("the video encoder stopped responding".into());
                        }
                    }
                    DequeuedOutputBufferInfoResult::OutputFormatChanged | DequeuedOutputBufferInfoResult::OutputBuffersChanged => {}
                }
            }
        }

        /// The next free input buffer, taking output while waiting so the encoder never stalls.
        fn queue(&mut self, frame: Option<&[u8]>) -> Result<(), String> {
            let start = Instant::now();
            loop {
                match self.codec.dequeue_input_buffer(WAIT).map_err(|e| err("input", e))? {
                    DequeuedInputBufferResult::Buffer(mut buf) => {
                        let pts = self.frame * 1_000_000 / self.fps;
                        let size = match frame {
                            Some(yuv) => {
                                let dst = buf.buffer_mut();
                                if dst.len() < yuv.len() {
                                    return Err(format!("the video encoder's buffer is too small ({} < {})", dst.len(), yuv.len()));
                                }
                                for (d, s) in dst.iter_mut().zip(yuv) {
                                    d.write(*s);
                                }
                                yuv.len()
                            }
                            None => 0,
                        };
                        let flags = if frame.is_none() { END_OF_STREAM } else { 0 };
                        self.codec.queue_input_buffer(buf, 0, size, pts, flags).map_err(|e| err("queue", e))?;
                        self.frame += 1;
                        return Ok(());
                    }
                    DequeuedInputBufferResult::TryAgainLater => {
                        self.drain(false)?;
                        if start.elapsed() > DRAIN_LIMIT {
                            return Err("the video encoder stopped taking frames".into());
                        }
                    }
                }
            }
        }
    }

    impl VideoEncoder for Encoder {
        fn push_rgba(&mut self, rgba: &[u8]) -> Result<(), String> {
            let mut yuv = std::mem::take(&mut self.yuv);
            let r = rgba_to_yuv420(rgba, &self.layout, &mut yuv).and_then(|()| self.queue(Some(&yuv)));
            self.yuv = yuv;
            r?;
            self.drain(false)
        }

        fn finish(mut self: Box<Self>) -> Result<Vec<u8>, String> {
            self.queue(None)?;
            self.drain(true)?;
            if !self.ended {
                return Err("the video encoder didn't finish the stream".into());
            }
            let _ = self.codec.stop();
            let Encoder { mux, .. } = *self;
            mux.finish()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_black_and_red_land_where_the_encoder_reads_them() {
        // 2 × 2: white, black / red, red.
        let rgba = [255, 255, 255, 255, 0, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255];
        let nv12 = Layout { width: 2, height: 2, stride: 4, slice_height: 4, semi_planar: true };
        let mut out = vec![0; nv12.frame_bytes()];
        rgba_to_yuv420(&rgba, &nv12, &mut out).unwrap();
        assert_eq!((out[0], out[1], out[4], out[5]), (235, 16, 82, 82), "luma, limited range");
        let (u, v) = (out[16], out[17]);
        assert!(v > 150 && u < 128, "reddish chroma: u {u} v {v}");
        let i420 = Layout { semi_planar: false, ..nv12 };
        let mut out2 = vec![0; i420.frame_bytes()];
        rgba_to_yuv420(&rgba, &i420, &mut out2).unwrap();
        assert_eq!((out2[16], out2[16 + 4]), (u, v), "the same chroma, in separate planes");
    }

    #[test]
    fn a_frame_that_does_not_fit_is_refused() {
        let l = Layout { width: 4, height: 4, stride: 4, slice_height: 4, semi_planar: true };
        assert!(rgba_to_yuv420(&[0; 8], &l, &mut [0; 24]).is_err());
        assert!(rgba_to_yuv420(&[0; 64], &l, &mut [0; 10]).is_err());
    }
}
