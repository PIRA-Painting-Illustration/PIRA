//! A minimal MP4 (ISO BMFF) writer for one H.264 video track: what a platform encoder produces
//! (Android's MediaCodec writes an Annex B stream: start codes, then SPS and PPS before the
//! first frame) wrapped as a file phones, browsers and social sites play. The index (`moov`)
//! comes before the media data, so a page can start playing it while it downloads.
//!
//! All samples go in one chunk, at a constant frame rate. Used by File › Timelapse.

/// Collects one H.264 stream and writes it as an `.mp4`.
#[derive(Debug, Clone)]
pub struct Mp4Writer {
    width: u32,
    height: u32,
    fps: u32,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
    data: Vec<u8>,
    sizes: Vec<u32>,
    /// 1-based numbers of the samples that start with a key frame.
    keys: Vec<u32>,
}

/// Ticks of the media timescale per frame.
const TICKS: u32 = 1000;

/// The NAL units of an Annex B buffer (without their start codes).
pub fn split_annex_b(buf: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= buf.len() {
        if buf[i] == 0 && buf[i + 1] == 0 && buf[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    for (k, &s) in starts.iter().enumerate() {
        let mut end = starts.get(k + 1).map_or(buf.len(), |&n| n - 3);
        // A four-byte start code's leading zero belongs to it, not to the unit before.
        while end > s && buf[end - 1] == 0 && k + 1 < starts.len() {
            end -= 1;
        }
        if end > s {
            out.push(&buf[s..end]);
        }
    }
    out
}

impl Mp4Writer {
    pub fn new(width: u32, height: u32, fps: u32) -> Self {
        Mp4Writer { width, height, fps: fps.max(1), sps: None, pps: None, data: Vec::new(), sizes: Vec::new(), keys: Vec::new() }
    }

    /// One encoder output buffer in Annex B form. SPS and PPS units become the track's
    /// configuration; the other units make one frame. `key` marks a key frame (an IDR unit in
    /// the buffer marks one too).
    pub fn push_annex_b(&mut self, buf: &[u8], key: bool) {
        let mut sample = Vec::new();
        let mut idr = false;
        for nal in split_annex_b(buf) {
            match nal[0] & 0x1F {
                7 => {
                    if self.sps.is_none() {
                        self.sps = Some(nal.to_vec());
                    }
                }
                8 => {
                    if self.pps.is_none() {
                        self.pps = Some(nal.to_vec());
                    }
                }
                // Access unit delimiters and filler carry nothing a player needs.
                9 | 12 => {}
                t => {
                    idr |= t == 5;
                    sample.extend_from_slice(&(nal.len() as u32).to_be_bytes());
                    sample.extend_from_slice(nal);
                }
            }
        }
        if sample.is_empty() {
            return;
        }
        self.data.extend_from_slice(&sample);
        self.sizes.push(sample.len() as u32);
        if key || idr {
            self.keys.push(self.sizes.len() as u32);
        }
    }

    pub fn frames(&self) -> usize {
        self.sizes.len()
    }

    /// The finished file.
    pub fn finish(self) -> Result<Vec<u8>, String> {
        let (Some(sps), Some(pps)) = (&self.sps, &self.pps) else {
            return Err("the video encoder gave no stream configuration (SPS/PPS)".into());
        };
        if sps.len() < 4 {
            return Err("the video encoder's SPS is too short".into());
        }
        if self.sizes.is_empty() {
            return Err("the video encoder gave no frames".into());
        }
        if self.data.len() > (u32::MAX - 16) as usize {
            return Err("the video is too large for this MP4 writer (4 GB)".into());
        }
        let ftyp = bx(b"ftyp", &cat(&[b"isom", &0x200u32.to_be_bytes(), b"isom", b"iso2", b"avc1", b"mp41"]));
        let moov_len = self.moov(sps, pps, 0).len();
        let offset = (ftyp.len() + moov_len + 8) as u32;
        let moov = self.moov(sps, pps, offset);
        let mut out = Vec::with_capacity(ftyp.len() + moov.len() + 8 + self.data.len());
        out.extend_from_slice(&ftyp);
        out.extend_from_slice(&moov);
        out.extend_from_slice(&((self.data.len() + 8) as u32).to_be_bytes());
        out.extend_from_slice(b"mdat");
        out.extend_from_slice(&self.data);
        Ok(out)
    }

    fn moov(&self, sps: &[u8], pps: &[u8], offset: u32) -> Vec<u8> {
        let n = self.sizes.len() as u32;
        let scale = self.fps * TICKS;
        let duration = n * TICKS;
        let matrix: [u32; 9] = [0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000];
        let matrix: Vec<u8> = matrix.iter().flat_map(|v| v.to_be_bytes()).collect();

        let mvhd = full(
            b"mvhd",
            0,
            0,
            &cat(&[&u32b(0), &u32b(0), &u32b(scale), &u32b(duration), &u32b(0x0001_0000), &0x0100u16.to_be_bytes(), &[0; 10], &matrix, &[0; 24], &u32b(2)]),
        );
        let tkhd = full(
            b"tkhd",
            0,
            3,
            &cat(&[
                &u32b(0),
                &u32b(0),
                &u32b(1),
                &u32b(0),
                &u32b(duration),
                &[0; 8],
                &[0; 4],
                &[0; 4],
                &matrix,
                &u32b(self.width << 16),
                &u32b(self.height << 16),
            ]),
        );
        let mdhd = full(b"mdhd", 0, 0, &cat(&[&u32b(0), &u32b(0), &u32b(scale), &u32b(duration), &0x55C4u16.to_be_bytes(), &[0, 0]]));
        let hdlr = full(b"hdlr", 0, 0, &cat(&[&u32b(0), b"vide", &[0; 12], b"VideoHandler\0"]));
        let vmhd = full(b"vmhd", 0, 1, &[0; 8]);
        let dinf = bx(b"dinf", &full(b"dref", 0, 0, &cat(&[&u32b(1), &full(b"url ", 0, 1, &[])])));

        let avcc = bx(
            b"avcC",
            &cat(&[&[1, sps[1], sps[2], sps[3], 0xFF, 0xE1], &(sps.len() as u16).to_be_bytes(), sps, &[1], &(pps.len() as u16).to_be_bytes(), pps]),
        );
        let mut name = [0u8; 32];
        let label = b"PIRA timelapse";
        name[0] = label.len() as u8;
        name[1..=label.len()].copy_from_slice(label);
        let avc1 = bx(
            b"avc1",
            &cat(&[
                &[0; 6],
                &1u16.to_be_bytes(),
                &[0; 16],
                &(self.width as u16).to_be_bytes(),
                &(self.height as u16).to_be_bytes(),
                &u32b(0x0048_0000),
                &u32b(0x0048_0000),
                &u32b(0),
                &1u16.to_be_bytes(),
                &name,
                &0x0018u16.to_be_bytes(),
                &0xFFFFu16.to_be_bytes(),
                &avcc,
            ]),
        );
        let stsd = full(b"stsd", 0, 0, &cat(&[&u32b(1), &avc1]));
        let stts = full(b"stts", 0, 0, &cat(&[&u32b(1), &u32b(n), &u32b(TICKS)]));
        let keys: Vec<u8> = self.keys.iter().flat_map(|k| k.to_be_bytes()).collect();
        let stss = full(b"stss", 0, 0, &cat(&[&u32b(self.keys.len() as u32), &keys]));
        let stsc = full(b"stsc", 0, 0, &cat(&[&u32b(1), &u32b(1), &u32b(n), &u32b(1)]));
        let sizes: Vec<u8> = self.sizes.iter().flat_map(|s| s.to_be_bytes()).collect();
        let stsz = full(b"stsz", 0, 0, &cat(&[&u32b(0), &u32b(n), &sizes]));
        let stco = full(b"stco", 0, 0, &cat(&[&u32b(1), &u32b(offset)]));
        let stbl = bx(b"stbl", &cat(&[&stsd, &stts, &stss, &stsc, &stsz, &stco]));
        let minf = bx(b"minf", &cat(&[&vmhd, &dinf, &stbl]));
        let mdia = bx(b"mdia", &cat(&[&mdhd, &hdlr, &minf]));
        let trak = bx(b"trak", &cat(&[&tkhd, &mdia]));
        bx(b"moov", &cat(&[&mvhd, &trak]))
    }
}

fn u32b(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

fn bx(tag: &[u8; 4], content: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(content.len() + 8);
    b.extend_from_slice(&((content.len() + 8) as u32).to_be_bytes());
    b.extend_from_slice(tag);
    b.extend_from_slice(content);
    b
}

fn full(tag: &[u8; 4], version: u8, flags: u32, content: &[u8]) -> Vec<u8> {
    let vf = (u32::from(version) << 24) | (flags & 0x00FF_FFFF);
    bx(tag, &cat(&[&vf.to_be_bytes(), content]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPS: &[u8] = &[0x67, 0x42, 0xC0, 0x1E, 0xDA, 0x01];
    const PPS: &[u8] = &[0x68, 0xCE, 0x3C, 0x80];

    fn annex(nals: &[&[u8]]) -> Vec<u8> {
        nals.iter().flat_map(|n| [&[0, 0, 0, 1][..], n].concat()).collect()
    }

    /// Top-level and nested boxes: (tag, payload).
    fn boxes(b: &[u8]) -> Vec<(String, &[u8])> {
        let mut out = Vec::new();
        let mut i = 0;
        while i + 8 <= b.len() {
            let len = u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
            out.push((String::from_utf8_lossy(&b[i + 4..i + 8]).into_owned(), &b[i + 8..i + len]));
            i += len;
        }
        out
    }

    fn find<'a>(b: &'a [u8], path: &[&str]) -> &'a [u8] {
        let mut cur = b;
        for p in path {
            cur = boxes(cur).into_iter().find(|(t, _)| t == p).unwrap_or_else(|| panic!("no {p}")).1;
        }
        cur
    }

    #[test]
    fn splits_three_and_four_byte_start_codes() {
        let buf = [0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3, 0, 0, 0, 1, 0x65, 4];
        assert_eq!(split_annex_b(&buf), vec![&[0x67, 1, 2][..], &[0x68, 3], &[0x65, 4]]);
    }

    #[test]
    fn writes_a_playable_layout_with_the_index_first() {
        let mut w = Mp4Writer::new(64, 48, 30);
        w.push_annex_b(&annex(&[SPS, PPS]), false);
        w.push_annex_b(&annex(&[&[0x65, 0xAA, 0xBB]]), true);
        w.push_annex_b(&annex(&[&[0x41, 0xCC]]), false);
        assert_eq!(w.frames(), 2);
        let mp4 = w.finish().unwrap();
        let top: Vec<String> = boxes(&mp4).into_iter().map(|(t, _)| t).collect();
        assert_eq!(top, ["ftyp", "moov", "mdat"]);
        // The chunk offset points at the first frame's length prefix.
        let stco = find(&mp4, &["moov", "trak", "mdia", "minf", "stbl", "stco"]);
        let off = u32::from_be_bytes([stco[8], stco[9], stco[10], stco[11]]) as usize;
        assert_eq!(&mp4[off..off + 7], &[0, 0, 0, 3, 0x65, 0xAA, 0xBB]);
        // Two samples of 7 and 6 bytes; the first is the only key frame.
        let stsz = find(&mp4, &["moov", "trak", "mdia", "minf", "stbl", "stsz"]);
        assert_eq!(&stsz[8..], &[0, 0, 0, 2, 0, 0, 0, 7, 0, 0, 0, 6]);
        let stss = find(&mp4, &["moov", "trak", "mdia", "minf", "stbl", "stss"]);
        assert_eq!(&stss[4..], &[0, 0, 0, 1, 0, 0, 0, 1]);
        // The decoder configuration carries the SPS and PPS.
        let stsd = find(&mp4, &["moov", "trak", "mdia", "minf", "stbl", "stsd"]);
        let avc1 = boxes(&stsd[8..])[0].1;
        let avcc = boxes(&avc1[78..]).into_iter().find(|(t, _)| t == "avcC").unwrap().1;
        assert_eq!(&avcc[..4], &[1, 0x42, 0xC0, 0x1E]);
        assert!(avcc.windows(SPS.len()).any(|w| w == SPS) && avcc.windows(PPS.len()).any(|w| w == PPS));
        // 2 frames at 30 fps.
        let mvhd = find(&mp4, &["moov", "mvhd"]);
        let (scale, dur) = (u32::from_be_bytes([mvhd[12], mvhd[13], mvhd[14], mvhd[15]]), u32::from_be_bytes([mvhd[16], mvhd[17], mvhd[18], mvhd[19]]));
        assert_eq!((scale, dur), (30_000, 2000));
    }

    #[test]
    fn refuses_a_stream_without_configuration_or_frames() {
        let mut w = Mp4Writer::new(16, 16, 30);
        w.push_annex_b(&annex(&[&[0x65, 1]]), true);
        assert!(w.finish().is_err());
        let mut w = Mp4Writer::new(16, 16, 30);
        w.push_annex_b(&annex(&[SPS, PPS]), false);
        assert!(w.finish().is_err());
    }
}

/// Wraps a real encoder's Annex B stream (`MP4_TEST_H264`, one access unit per frame as an
/// encoder hands them over) into `MP4_TEST_OUT`, for checking with a player or `ffprobe`.
#[cfg(test)]
#[test]
#[ignore = "needs MP4_TEST_H264 and MP4_TEST_OUT"]
fn wraps_a_real_stream() {
    let (Ok(input), Ok(out)) = (std::env::var("MP4_TEST_H264"), std::env::var("MP4_TEST_OUT")) else { return };
    let bytes = std::fs::read(input).unwrap();
    let mut w = Mp4Writer::new(640, 352, 30);
    // Regroup the units into access units: a new one starts at each slice whose first_mb is 0
    // (here: every slice, one per frame), with parameter sets riding along.
    let mut unit = Vec::new();
    for nal in split_annex_b(&bytes) {
        let t = nal[0] & 0x1F;
        if (t == 1 || t == 5) && !unit.is_empty() && unit.iter().any(|n: &&[u8]| matches!(n[0] & 0x1F, 1 | 5)) {
            let buf: Vec<u8> = unit.iter().flat_map(|n: &&[u8]| [&[0, 0, 0, 1][..], n].concat()).collect();
            w.push_annex_b(&buf, false);
            unit.clear();
        }
        unit.push(nal);
    }
    let buf: Vec<u8> = unit.iter().flat_map(|n| [&[0, 0, 0, 1][..], n].concat()).collect();
    w.push_annex_b(&buf, false);
    std::fs::write(out, w.finish().unwrap()).unwrap();
}
