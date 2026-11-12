//! File › Timelapse: the recording setting and every snapshot survive a `.pcraft` round trip; a
//! file without a timelapse loads with an empty log that isn't recording.

use std::sync::Arc;

use photocraft_color::{ColorMode, SampleType};
use photocraft_doc::{Document, Size, TimelapseFrame};
use photocraft_format::*;

#[test]
fn the_timelapse_roundtrips_and_is_absent_from_files_without_one() {
    let doc = Document::new("a", Size::new(40, 30), ColorMode::Rgb, SampleType::U8);
    let plain = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    let back = load_from_bytes(&plain).unwrap();
    assert!(!back.timelapse.recording() && back.timelapse.is_empty());
    assert!(!String::from_utf8_lossy(&plain).contains("\"timelapse\""), "no timelapse entry in an ordinary file");

    doc.timelapse.set_recording(true);
    for n in 0..3u8 {
        doc.timelapse.push(TimelapseFrame { width: 40, height: 30, jpeg: Arc::new(vec![0xFF, 0xD8, n]) });
    }
    let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    let back = load_from_bytes(&bytes).unwrap();
    assert!(back.timelapse.recording(), "recording carries on after reopening");
    assert_eq!(back.timelapse.frames(), doc.timelapse.frames());
    assert!(!back.timelapse.same_log(&doc.timelapse), "a reopened file has a log of its own");
}
