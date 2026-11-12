use super::*;
use photocraft_engine::timelapse_cmds::{VideoEncoder, VideoEncoderFactory};
use std::sync::Mutex;

struct Counter(usize);
impl VideoEncoder for Counter {
    fn push_rgba(&mut self, _: &[u8]) -> Result<(), String> {
        self.0 += 1;
        Ok(())
    }
    fn finish(self: Box<Self>) -> Result<Vec<u8>, String> {
        Ok(format!("{} frames", self.0).into_bytes())
    }
}

/// Files written: path and bytes.
type Written = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

fn app(encoder: bool) -> (PhotocraftApp, Written) {
    let written: Written = Arc::default();
    let w = written.clone();
    let make: VideoEncoderFactory = Arc::new(|_, _, _| Ok(Box::new(Counter(0)) as Box<dyn VideoEncoder>));
    let services = crate::Services {
        write: Some(Box::new(move |p: &str, b: &[u8]| {
            w.lock().unwrap().push((p.to_string(), b.to_vec()));
            Ok(())
        })),
        video_encoder: encoder.then_some(make),
        ..Default::default()
    };
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), services);
    app.run("file.new", json!({"width": 40, "height": 30, "timelapse": true})).unwrap();
    (app, written)
}

fn take_snapshots(app: &mut PhotocraftApp) {
    for job in app.session.timelapse_jobs() {
        job.run();
    }
}

#[test]
fn export_needs_an_encoder_and_a_recording() {
    let (mut a, _) = app(false);
    take_snapshots(&mut a);
    assert!(open(&mut a).unwrap_err().contains("isn't available"));
    let (mut a, _) = app(true);
    assert!(open(&mut a).unwrap_err().contains("Record Timelapse"), "nothing recorded yet");
    take_snapshots(&mut a);
    assert!(open(&mut a).is_ok());
    assert!(a.ui.timelapse_export.is_some());
}

#[test]
fn the_record_menu_item_shows_whether_the_canvas_records() {
    let (mut a, _) = app(true);
    assert_eq!(crate::menus::is_checked(&a, "file.timelapse.record"), Some(true));
    a.run("file.timelapse.record", json!({})).unwrap();
    assert_eq!(crate::menus::is_checked(&a, "file.timelapse.record"), Some(false));
}

#[test]
fn an_export_runs_on_a_worker_and_writes_the_file() {
    let (mut a, written) = app(true);
    take_snapshots(&mut a);
    open(&mut a).unwrap();
    let opts = ExportOptions { seconds: Some(1), side: 720, aspect: Aspect::Wide };
    start(&mut a, opts, "/tmp/cat timelapse.mp4".into()).unwrap();
    for _ in 0..500 {
        finish(&mut a);
        if a.ui.timelapse_export.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(a.ui.timelapse_export.is_none(), "the window closes when it is done");
    let w = written.lock().unwrap();
    assert_eq!(w.len(), 1);
    let frames = (tlc::FPS + tlc::HOLD_SECONDS * tlc::FPS) as usize;
    assert_eq!(w[0], ("/tmp/cat timelapse.mp4".to_string(), format!("{frames} frames").into_bytes()));
    assert!(a.ui.status.contains("cat timelapse.mp4") && !a.ui.status_error, "{}", a.ui.status);
}
