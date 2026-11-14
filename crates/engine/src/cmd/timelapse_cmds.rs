//! File › Timelapse, as in Clip Studio Paint: Record Timelapse keeps a snapshot of the canvas
//! after each change (the log lives in the document, [`photocraft_doc::timelapse`]), and Export
//! Timelapse turns the snapshots into a sped-up video of a chosen length, size and shape.
//!
//! Snapshots are taken off the UI thread: the app asks [`Session::timelapse_jobs`] each frame
//! and runs what it gets on a worker. A document is snapshotted when its revision has moved since
//! the last one (an edit, an undo, a redo), at most every [`MIN_GAP`], one at a time, so a burst
//! of strokes costs one snapshot rather than one each.
//!
//! The video itself is encoded by the platform ([`VideoEncoder`]): Android's MediaCodec, the
//! system's own H.264 encoder, so the app carries no codec of its own.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use photocraft_doc::{Document, TimelapseFrame, timelapse::FRAME_SIDE};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::commands::CommandSpec;
use crate::{EngineError, Result, Session};

/// Shortest time between two snapshots of one document.
pub const MIN_GAP: std::time::Duration = std::time::Duration::from_millis(400);
/// JPEG quality of a snapshot (1–100).
const QUALITY: u8 = 80;
/// Video frame rate.
pub const FPS: u32 = 30;
/// How long the finished picture stays on screen at the end.
pub const HOLD_SECONDS: u32 = 2;
/// Export lengths offered (seconds); `None` in [`ExportOptions::seconds`] is All (every snapshot
/// once).
pub const LENGTHS: [u32; 3] = [15, 30, 60];
/// Export sizes offered: the video's longer side, in pixels.
pub const SIZES: [u32; 3] = [720, 1080, 1280];

/// A snapshot to take on a worker thread ([`Session::timelapse_jobs`]).
pub struct SnapshotJob {
    doc: Arc<Document>,
    busy: Arc<AtomicBool>,
}

impl SnapshotJob {
    /// Takes the snapshot and adds it to the document's log (unless the canvas looks just as it
    /// did in the last one).
    pub fn run(self) {
        if let Some(frame) = snapshot(&self.doc) {
            let log = &self.doc.timelapse;
            if log.last().is_none_or(|last| last.jpeg != frame.jpeg) {
                log.push(frame);
            }
        }
        self.busy.store(false, Ordering::Release);
    }
}

/// The flattened canvas, scaled to fit [`FRAME_SIDE`] and laid on white (a video has no
/// transparency), as a JPEG.
pub fn snapshot(doc: &Document) -> Option<TimelapseFrame> {
    let img = photocraft_compose::thumbnail(doc, FRAME_SIDE);
    let (w, h) = (img.width, img.height);
    if w == 0 || h == 0 {
        return None;
    }
    let rgb = on_white(&img.pixels);
    let jpeg = photocraft_codecs::web::encode_jpeg_rgb8(w, h, &rgb, QUALITY, false, false, None, None, None, None).ok()?;
    Some(TimelapseFrame { width: w, height: h, jpeg: Arc::new(jpeg) })
}

/// The document composited over white at most `side` pixels on its longer side, as opaque RGBA
/// with its width and height.
pub fn composite_on_white(doc: &Document, side: u32) -> Option<(Vec<u8>, u32, u32)> {
    let img = photocraft_compose::thumbnail(doc, side);
    let (w, h) = (img.width, img.height);
    if w == 0 || h == 0 {
        return None;
    }
    let rgba = on_white(&img.pixels).as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
    Some((rgba, w, h))
}

/// Straight-alpha RGBA over white, as RGB.
fn on_white(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len() / 4 * 3);
    for p in rgba.as_chunks::<4>().0 {
        let a = u32::from(p[3]);
        for &c in &p[..3] {
            out.push(((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8);
        }
    }
    out
}

impl Session {
    /// The snapshots due now: documents that record, changed since their last snapshot, have
    /// none in progress and had none in the last [`MIN_GAP`]. Run each on a worker thread.
    pub fn timelapse_jobs(&mut self) -> Vec<SnapshotJob> {
        let now = web_time::Instant::now();
        let mut jobs = Vec::new();
        for st in self.docs.iter_mut() {
            if st.timelapse_seen == st.revision || !st.doc.timelapse.recording() {
                continue;
            }
            if st.timelapse_busy.load(Ordering::Acquire) || st.timelapse_at.is_some_and(|t| now.duration_since(t) < MIN_GAP) {
                continue;
            }
            st.timelapse_seen = st.revision;
            st.timelapse_at = Some(now);
            st.timelapse_busy.store(true, Ordering::Release);
            jobs.push(SnapshotJob { doc: st.doc.clone(), busy: st.timelapse_busy.clone() });
        }
        jobs
    }

    /// Whether a recording document has changed since its last snapshot (the app keeps frames
    /// coming until it is taken).
    pub fn timelapse_waiting(&self) -> bool {
        self.docs.iter().any(|st| st.timelapse_seen != st.revision && st.doc.timelapse.recording())
    }
}

fn has_doc(s: &Session) -> std::result::Result<(), String> {
    s.active().map(|_| ()).ok_or_else(|| "no document open".into())
}

/// File › Timelapse › Record Timelapse: `{on}` sets it, no `on` toggles. Off deletes the
/// recording, for good.
fn record(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let on = match p.get("on") {
        None | Some(Value::Null) => !st.doc.timelapse.recording(),
        Some(v) => v.as_bool().ok_or_else(|| EngineError::BadParams { cmd: "file.timelapse.record".into(), msg: "`on` must be true or false".into() })?,
    };
    // Not a history step (the log is shared by every state), but the file needs saving.
    s.edit_with("Record Timelapse", false, |doc, _| {
        doc.timelapse.set_recording(on);
        Ok(())
    })?;
    if on && let Some(st) = s.active_mut() {
        // The canvas as it is now is the first snapshot.
        st.timelapse_seen = 0;
    }
    info(s, &Value::Null)
}

fn info(s: &mut Session, _: &Value) -> Result<Value> {
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let log = &st.doc.timelapse;
    Ok(json!({"recording": log.recording(), "frames": log.len(), "bytes": log.bytes()}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "file.timelapse.record",
            label: "Record Timelapse",
            menu: &["File", "Timelapse"],
            shortcut: None,
            params: r#"{"on":bool? (absent: toggle; false deletes the recording)} → {recording, frames, bytes}"#,
            enabled: has_doc,
            run: record,
            journal: true,
        },
        CommandSpec {
            id: "file.timelapse.info",
            label: "Timelapse Info",
            menu: &[],
            shortcut: None,
            params: "{} → {recording, frames, bytes}",
            enabled: has_doc,
            run: info,
            journal: false,
        },
    ]
}

// ------------------------------------------------------------------------------------- export

/// The video's shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Aspect {
    /// The canvas's own shape.
    #[default]
    Original,
    Wide,
    Standard,
    Square,
    Portrait,
    Tall,
}

impl Aspect {
    pub const ALL: [Aspect; 6] = [Aspect::Original, Aspect::Wide, Aspect::Standard, Aspect::Square, Aspect::Portrait, Aspect::Tall];

    pub fn label(self) -> &'static str {
        match self {
            Aspect::Original => "Original",
            Aspect::Wide => "16:9",
            Aspect::Standard => "4:3",
            Aspect::Square => "1:1",
            Aspect::Portrait => "4:5",
            Aspect::Tall => "9:16",
        }
    }

    /// Width : height, or `None` for the canvas's.
    pub fn ratio(self) -> Option<(u32, u32)> {
        match self {
            Aspect::Original => None,
            Aspect::Wide => Some((16, 9)),
            Aspect::Standard => Some((4, 3)),
            Aspect::Square => Some((1, 1)),
            Aspect::Portrait => Some((4, 5)),
            Aspect::Tall => Some((9, 16)),
        }
    }
}

/// File › Timelapse › Export Timelapse's settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportOptions {
    /// The video's length without the closing hold; `None`: All (each snapshot once).
    pub seconds: Option<u32>,
    /// The longer side in pixels.
    pub side: u32,
    pub aspect: Aspect,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions { seconds: Some(30), side: 1080, aspect: Aspect::Original }
    }
}

/// The video's size for a canvas of `canvas` pixels: the longer side `side`, the other from the
/// aspect, both multiples of 16 (what hardware H.264 encoders take without padding).
pub fn output_size(canvas: (u32, u32), side: u32, aspect: Aspect) -> (u32, u32) {
    let (rw, rh) = aspect.ratio().unwrap_or((canvas.0.max(1), canvas.1.max(1)));
    let side = f64::from(side.clamp(64, 4096));
    let (w, h) = if rw >= rh { (side, side * f64::from(rh) / f64::from(rw)) } else { (side * f64::from(rw) / f64::from(rh), side) };
    let even = |v: f64| ((v / 16.0).round() as u32).max(1) * 16;
    (even(w), even(h))
}

/// Which snapshot each video frame shows, and how many frames the finished picture holds at the
/// end. A fixed length spreads the snapshots over it (each shown for several frames when there
/// are few, some skipped when there are many); All shows each once.
pub fn plan_frames(recorded: usize, seconds: Option<u32>) -> (Vec<usize>, usize) {
    let hold = (HOLD_SECONDS * FPS) as usize;
    if recorded == 0 {
        return (Vec::new(), hold);
    }
    let body = match seconds {
        None => recorded,
        Some(s) => (s.max(1) * FPS) as usize,
    };
    ((0..body).map(|i| (i * recorded / body).min(recorded - 1)).collect(), hold)
}

/// A video encoder the platform provides: frames go in as RGBA8 of the size it was made for,
/// and the finished file (an MP4) comes out. It is made and used on one thread (the export's
/// worker): a platform codec may be tied to the thread that created it.
pub trait VideoEncoder {
    fn push_rgba(&mut self, rgba: &[u8]) -> std::result::Result<(), String>;
    fn finish(self: Box<Self>) -> std::result::Result<Vec<u8>, String>;
}

/// Makes an encoder for `width` × `height` frames at `fps`.
pub type VideoEncoderFactory = Arc<dyn Fn(u32, u32, u32) -> std::result::Result<Box<dyn VideoEncoder>, String> + Send + Sync>;

/// Renders the timelapse of `doc` and encodes it with `make`. `progress(done, total)` returns
/// false to cancel. Returns the video file's bytes.
pub fn export(
    doc: &Document,
    opts: &ExportOptions,
    make: &VideoEncoderFactory,
    mut progress: impl FnMut(usize, usize) -> bool,
) -> std::result::Result<Vec<u8>, String> {
    let frames = doc.timelapse.frames();
    if frames.is_empty() {
        return Err("This canvas has no timelapse recorded yet. Turn on File › Timelapse › Record Timelapse and paint.".into());
    }
    let canvas = (doc.size.width, doc.size.height);
    let (w, h) = output_size(canvas, opts.side, opts.aspect);
    let (plan, hold) = plan_frames(frames.len(), opts.seconds);
    let total = plan.len() + hold;
    let mut enc = make(w, h, FPS)?;
    let mut decoded: Option<(usize, Vec<u8>)> = None;
    for (n, &i) in plan.iter().enumerate() {
        if decoded.as_ref().is_none_or(|(at, _)| *at != i) {
            let f = &frames[i];
            let rgba = decode_rgba(&f.jpeg, f.width, f.height)?;
            decoded = Some((i, fit(&rgba, f.width, f.height, w, h)));
        }
        if let Some((_, px)) = &decoded {
            enc.push_rgba(px)?;
        }
        if !progress(n + 1, total) {
            return Err("cancelled".into());
        }
    }
    // The finished picture, sharp, from the document itself.
    let last = snapshot(doc).ok_or("the canvas could not be drawn")?;
    let px = fit(&decode_rgba(&last.jpeg, last.width, last.height)?, last.width, last.height, w, h);
    for k in 0..hold {
        enc.push_rgba(&px)?;
        if !progress(plan.len() + k + 1, total) {
            return Err("cancelled".into());
        }
    }
    enc.finish()
}

fn decode_rgba(jpeg: &[u8], w: u32, h: u32) -> std::result::Result<Vec<u8>, String> {
    let img = photocraft_codecs::decode(jpeg).map_err(|e| format!("a timelapse snapshot could not be read: {e}"))?;
    if img.width() != w || img.height() != h {
        return Err("a timelapse snapshot is damaged".into());
    }
    Ok(img.to_rgba8())
}

/// `src` (`sw` × `sh` RGBA8) scaled to fit inside `w` × `h`, centred on black (Clip Studio Paint
/// fills the rest of another shape with black too), with bilinear filtering.
pub fn fit(src: &[u8], sw: u32, sh: u32, w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (w * h * 4) as usize];
    for p in out.as_chunks_mut::<4>().0 {
        p[3] = 255;
    }
    if sw == 0 || sh == 0 || src.len() < (sw * sh * 4) as usize {
        return out;
    }
    let scale = (f64::from(w) / f64::from(sw)).min(f64::from(h) / f64::from(sh));
    let (dw, dh) = (((f64::from(sw) * scale).round() as u32).clamp(1, w), ((f64::from(sh) * scale).round() as u32).clamp(1, h));
    let (ox, oy) = ((w - dw) / 2, (h - dh) / 2);
    let px = |x: u32, y: u32, c: usize| f64::from(src[((y * sw + x) * 4) as usize + c]);
    for y in 0..dh {
        let fy = ((f64::from(y) + 0.5) / scale - 0.5).clamp(0.0, f64::from(sh - 1));
        let (y0, ty) = (fy.floor() as u32, fy - fy.floor());
        let y1 = (y0 + 1).min(sh - 1);
        for x in 0..dw {
            let fx = ((f64::from(x) + 0.5) / scale - 0.5).clamp(0.0, f64::from(sw - 1));
            let (x0, tx) = (fx.floor() as u32, fx - fx.floor());
            let x1 = (x0 + 1).min(sw - 1);
            let o = (((oy + y) * w + ox + x) * 4) as usize;
            for c in 0..3 {
                let top = px(x0, y0, c) * (1.0 - tx) + px(x1, y0, c) * tx;
                let bottom = px(x0, y1, c) * (1.0 - tx) + px(x1, y1, c) * tx;
                out[o + c] = (top * (1.0 - ty) + bottom * ty).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
