use super::*;
use photocraft_engine::timelapse_cmds::{VideoEncoder, VideoEncoderFactory};
use std::sync::{Arc, Mutex};

/// Frames pushed, and the size and rate the encoder was made with.
type Seen = Arc<Mutex<Vec<(u32, u32, u32, usize)>>>;
type Written = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

struct Counter(Seen, usize);
impl VideoEncoder for Counter {
    fn push_rgba(&mut self, px: &[u8]) -> Result<(), String> {
        let mut seen = self.0.lock().unwrap();
        let (w, h, ..) = seen[0];
        assert_eq!(px.len(), (w * h * 4) as usize, "a whole frame");
        seen[0].3 += 1;
        self.1 += 1;
        Ok(())
    }
    fn finish(self: Box<Self>) -> Result<Vec<u8>, String> {
        Ok(format!("{} frames", self.1).into_bytes())
    }
}

fn app() -> (PhotocraftApp, Seen, Written) {
    let seen: Seen = Arc::default();
    let s = seen.clone();
    let make: VideoEncoderFactory = Arc::new(move |w, h, fps| {
        s.lock().unwrap().push((w, h, fps, 0));
        Ok(Box::new(Counter(s.clone(), 0)) as Box<dyn VideoEncoder>)
    });
    let written: Written = Arc::default();
    let w = written.clone();
    let services = crate::Services {
        write: Some(Box::new(move |p: &str, b: &[u8]| {
            w.lock().unwrap().push((p.to_string(), b.to_vec()));
            Ok(())
        })),
        video_encoder: Some(make),
        ..Default::default()
    };
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), services);
    app.run("file.new", json!({"width": 64, "height": 48})).unwrap();
    (app, seen, written)
}

#[test]
fn every_timeline_frame_goes_into_one_mp4() {
    let (mut app, seen, written) = app();
    app.run("timeline.create", json!({"duration": 6, "fps": 12})).unwrap();
    render_to(&mut app, "/out/clip.mp4").unwrap();
    let (w, h, fps, pushed) = seen.lock().unwrap()[0];
    assert_eq!((w % 16, h % 16, fps, pushed), (0, 0, 12, 6), "{w}x{h}");
    let written = written.lock().unwrap();
    assert_eq!(written.as_slice(), [("/out/clip.mp4".to_string(), b"6 frames".to_vec())]);
    assert!(!app.ui.status_error && app.ui.status.contains("clip.mp4"), "{}", app.ui.status);
}

#[test]
fn a_gif_name_saves_an_animated_gif_and_no_timeline_is_one_frame() {
    let (mut app, seen, written) = app();
    render_to(&mut app, "/out/clip.GIF").unwrap();
    assert!(seen.lock().unwrap().is_empty(), "no video encoder for a GIF");
    let written = written.lock().unwrap();
    assert!(written[0].1.starts_with(b"GIF8"), "a GIF file");
}

#[test]
fn no_encoder_still_offers_a_gif() {
    let (mut app, ..) = app();
    app.services.video_encoder = None;
    assert!(render_to(&mut app, "/out/clip.mp4").unwrap_err().contains(".gif"));
}
