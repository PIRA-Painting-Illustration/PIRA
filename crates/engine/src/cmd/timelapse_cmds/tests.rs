use super::*;
use serde_json::json;

fn session_recording() -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 64, "height": 48, "timelapse": true})).unwrap();
    s
}

fn run_jobs(s: &mut Session) -> usize {
    let jobs = s.timelapse_jobs();
    let n = jobs.len();
    for j in jobs {
        j.run();
    }
    n
}

fn frames(s: &Session) -> usize {
    s.active().unwrap().doc.timelapse.len()
}

fn paint(s: &mut Session, x: f64) {
    s.execute("paint.stroke", json!({"points": [[x, 10.0], [x + 20.0, 30.0]], "size": 8, "color": "#1a33e6"})).unwrap();
}

#[test]
fn a_new_canvas_can_record_from_the_start_and_each_change_is_snapshotted() {
    let mut s = session_recording();
    assert_eq!(run_jobs(&mut s), 1, "the blank canvas is the first snapshot");
    assert_eq!(frames(&s), 1);
    assert_eq!(run_jobs(&mut s), 0, "nothing changed");
    paint(&mut s, 5.0);
    // Within the minimum gap: wait for the next one.
    assert_eq!(run_jobs(&mut s), 0);
    s.active_mut().unwrap().timelapse_at = None;
    assert_eq!(run_jobs(&mut s), 1);
    assert_eq!(frames(&s), 2);
    let f = s.active().unwrap().doc.timelapse.last().unwrap();
    assert_eq!((f.width, f.height), (64, 48), "a small canvas keeps its size");
}

#[test]
fn undo_keeps_the_snapshots_and_is_recorded_too() {
    let mut s = session_recording();
    run_jobs(&mut s);
    paint(&mut s, 5.0);
    s.active_mut().unwrap().timelapse_at = None;
    run_jobs(&mut s);
    s.execute("edit.undo", json!({})).unwrap();
    s.active_mut().unwrap().timelapse_at = None;
    run_jobs(&mut s);
    // The undone canvas looks like the first snapshot, but it is a new moment in the painting.
    assert_eq!(frames(&s), 3);
}

#[test]
fn recording_off_deletes_it_and_a_canvas_that_does_not_record_takes_none() {
    let mut s = session_recording();
    run_jobs(&mut s);
    let r = s.execute("file.timelapse.record", json!({"on": false})).unwrap();
    assert_eq!(r["recording"], false);
    assert_eq!(frames(&s), 0);
    paint(&mut s, 5.0);
    assert_eq!(run_jobs(&mut s), 0);
    // Toggle back on: the canvas as it is becomes the first snapshot.
    let r = s.execute("file.timelapse.record", json!({})).unwrap();
    assert_eq!(r["recording"], true);
    s.active_mut().unwrap().timelapse_at = None;
    assert_eq!(run_jobs(&mut s), 1);
}

#[test]
fn output_sizes_follow_the_aspect_in_multiples_of_16() {
    assert_eq!(output_size((3000, 2000), 1280, Aspect::Wide), (1280, 720));
    assert_eq!(output_size((3000, 2000), 1080, Aspect::Tall), (608, 1088));
    assert_eq!(output_size((3000, 2000), 720, Aspect::Square), (720, 720));
    assert_eq!(output_size((3000, 2000), 1280, Aspect::Original), (1280, 848));
    let (w, h) = output_size((7, 3000), 720, Aspect::Original);
    assert!(w >= 16 && h == 720 && w % 16 == 0);
}

#[test]
fn a_fixed_length_spreads_the_snapshots_and_all_shows_each_once() {
    let (p, hold) = plan_frames(10, Some(1));
    assert_eq!(p.len(), FPS as usize);
    assert_eq!((p[0], p[p.len() - 1]), (0, 9));
    assert!(p.windows(2).all(|w| w[0] <= w[1]), "in order");
    assert_eq!(hold, (HOLD_SECONDS * FPS) as usize);
    let (p, _) = plan_frames(5000, Some(15));
    assert_eq!(p.len(), 450);
    assert_eq!(p[449], 4988);
    assert_eq!(plan_frames(7, None).0, vec![0, 1, 2, 3, 4, 5, 6]);
    assert!(plan_frames(0, Some(30)).0.is_empty());
}

#[test]
fn fit_letterboxes_on_black() {
    // A white 4 × 2 image in an 8 × 8 frame: white band in the middle, black above and below.
    let src = vec![255u8; 4 * 2 * 4];
    let out = fit(&src, 4, 2, 8, 8);
    let at = |x: usize, y: usize| out[(y * 8 + x) * 4];
    assert_eq!((at(4, 0), at(4, 4), at(4, 7)), (0, 255, 0));
    assert!(out.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
}

struct Collect(Vec<usize>, (u32, u32));
impl VideoEncoder for Collect {
    fn push_rgba(&mut self, rgba: &[u8]) -> std::result::Result<(), String> {
        assert_eq!(rgba.len(), (self.1.0 * self.1.1 * 4) as usize);
        self.0.push(rgba.len());
        Ok(())
    }
    fn finish(self: Box<Self>) -> std::result::Result<Vec<u8>, String> {
        Ok(vec![self.0.len() as u8])
    }
}

#[test]
fn export_sends_every_planned_frame_and_the_hold_to_the_encoder() {
    let mut s = session_recording();
    run_jobs(&mut s);
    paint(&mut s, 5.0);
    s.active_mut().unwrap().timelapse_at = None;
    run_jobs(&mut s);
    let doc = s.active().unwrap().doc.clone();
    let make: VideoEncoderFactory = Arc::new(|w, h, fps| {
        assert_eq!(fps, FPS);
        Ok(Box::new(Collect(Vec::new(), (w, h))) as Box<dyn VideoEncoder>)
    });
    let opts = ExportOptions { seconds: None, side: 720, aspect: Aspect::Square };
    let mut last = (0, 0);
    let out = export(&doc, &opts, &make, |d, t| {
        last = (d, t);
        true
    })
    .unwrap();
    let expected = 2 + (HOLD_SECONDS * FPS) as usize;
    assert_eq!(out, vec![expected as u8]);
    assert_eq!(last, (expected, expected));
    // Nothing recorded: a clear message, no video.
    let mut s2 = Session::new();
    s2.execute("file.new", json!({"width": 16, "height": 16})).unwrap();
    assert!(export(&s2.active().unwrap().doc, &opts, &make, |_, _| true).unwrap_err().contains("Record Timelapse"));
}
