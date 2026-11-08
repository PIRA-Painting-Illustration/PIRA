use super::*;
use crate::canvas::tool_event;
use crate::state::Tool;

/// An app with a 400 × 300 document open.
fn app() -> PhotocraftApp {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    app.session.tools.brush.smoothing.amount = 0.0;
    app
}

fn drag(app: &mut PhotocraftApp, pts: &[[f64; 2]]) {
    let mods = egui::Modifiers::NONE;
    tool_event(app, ToolEvent::Down { x: pts[0][0], y: pts[0][1], pressure: 1.0 }, mods);
    for p in &pts[1..pts.len() - 1] {
        tool_event(app, ToolEvent::Move { x: p[0], y: p[1], pressure: 1.0 }, mods);
    }
    let last = pts[pts.len() - 1];
    tool_event(app, ToolEvent::Up { x: last[0], y: last[1] }, mods);
}

fn rulers(app: &PhotocraftApp) -> Vec<RulerShape> {
    app.session.active().unwrap().doc.pira.rulers.iter().map(|r| r.shape.clone()).collect()
}

fn last_stroke(app: &PhotocraftApp) -> Vec<[f64; 2]> {
    let p = app.session.journal.iter().rev().find(|(id, _)| id == "paint.stroke").map(|(_, p)| p.clone()).unwrap();
    p["points"].as_array().unwrap().iter().map(|q| [q[0].as_f64().unwrap(), q[1].as_f64().unwrap()]).collect()
}

#[test]
fn drags_make_each_kind_of_ruler() {
    let line = [[10.0, 10.0], [50.0, 30.0], [100.0, 10.0]];
    assert_eq!(shape_from_drag(1, &line, 8.0), Some(RulerShape::Straight { a: [10.0, 10.0], b: [100.0, 10.0] }));
    assert!(matches!(shape_from_drag(2, &line, 8.0), Some(RulerShape::Curve { points }) if points.len() == 3));
    assert_eq!(
        shape_from_drag(3, &[[100.0, 100.0], [160.0, 130.0]], 8.0),
        Some(RulerShape::Concentric { center: [100.0, 100.0], radii: [60.0, 30.0], angle: 0.0 })
    );
    assert!(matches!(shape_from_drag(3, &[[0.0, 0.0], [50.0, 2.0]], 8.0), Some(RulerShape::Concentric { radii, .. }) if radii[0] == radii[1]), "a circle");
    assert_eq!(shape_from_drag(5, &[[20.0, 20.0], [20.0, 20.0]], 8.0), Some(RulerShape::Radial { center: [20.0, 20.0] }), "a click is enough");
    assert_eq!(shape_from_drag(4, &[[20.0, 20.0], [22.0, 20.0]], 8.0), None, "too short to have a direction");
    // Handles: an ellipse's axis handles turn it and set its radii.
    let e = RulerShape::Concentric { center: [0.0, 0.0], radii: [10.0, 5.0], angle: 0.0 };
    assert_eq!(handles(&e), vec![[0.0, 0.0], [10.0, 0.0], [0.0, 5.0]]);
    let RulerShape::Concentric { radii, angle, .. } = move_handle(&e, 1, [0.0, 20.0]) else { return };
    assert!((radii[0] - 20.0).abs() < 1e-9 && (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
}

#[test]
fn the_ruler_tool_draws_a_straight_ruler_that_strokes_near_it_follow() {
    let mut app = app();
    app.ui.tool = Tool::Ruler;
    app.ui.tool_options.special_ruler = 1;
    drag(&mut app, &[[20.0, 100.0], [200.0, 100.0], [380.0, 100.0]]);
    assert_eq!(rulers(&app), vec![RulerShape::Straight { a: [20.0, 100.0], b: [380.0, 100.0] }]);
    assert!(app.session.active().unwrap().doc.measurement.ruler.is_none(), "it didn't measure");
    // A wobbly stroke starting near it runs along it, its first point included.
    app.ui.tool = Tool::Brush;
    drag(&mut app, &[[50.0, 110.0], [70.0, 112.0], [100.0, 95.0], [150.0, 108.0], [200.0, 104.0]]);
    let pts = last_stroke(&app);
    assert!(pts.len() >= 3 && pts.iter().all(|p| (p[1] - 100.0).abs() < 1e-6), "{pts:?}");
    // One far away draws freely.
    drag(&mut app, &[[50.0, 250.0], [80.0, 240.0], [120.0, 260.0]]);
    assert_eq!(last_stroke(&app).last(), Some(&[120.0, 260.0]));
    // Snap off: free everywhere.
    app.run("ruler.snap", json!({})).unwrap();
    drag(&mut app, &[[50.0, 110.0], [80.0, 115.0], [120.0, 90.0]]);
    assert_eq!(last_stroke(&app).last(), Some(&[120.0, 90.0]));
}

#[test]
fn a_radial_ruler_sends_strokes_towards_its_centre() {
    let mut app = app();
    app.ui.tool = Tool::Ruler;
    app.ui.tool_options.special_ruler = 5;
    drag(&mut app, &[[200.0, 150.0], [200.0, 150.0]]);
    app.ui.tool = Tool::Brush;
    drag(&mut app, &[[20.0, 150.0], [40.0, 156.0], [80.0, 140.0], [120.0, 160.0]]);
    assert!(last_stroke(&app).iter().all(|p| (p[1] - 150.0).abs() < 1e-6));
}

#[test]
fn a_handle_drag_reshapes_a_ruler_in_one_undoable_step() {
    let mut app = app();
    app.ui.tool = Tool::Ruler;
    app.ui.tool_options.special_ruler = 4;
    drag(&mut app, &[[100.0, 100.0], [200.0, 100.0]]);
    // With the kind back on Measure, a press on a handle still grabs it.
    app.ui.tool_options.special_ruler = 0;
    drag(&mut app, &[[200.0, 101.0], [210.0, 150.0], [200.0, 200.0]]);
    assert_eq!(rulers(&app), vec![RulerShape::Parallel { a: [100.0, 100.0], b: [200.0, 200.0] }]);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(rulers(&app), vec![RulerShape::Parallel { a: [100.0, 100.0], b: [200.0, 100.0] }]);
    // Away from the handles, with Measure, the press is left to measure.
    assert!(!pointer(&mut app, ToolEvent::Down { x: 10.0, y: 10.0, pressure: 1.0 }));
}

#[test]
fn every_kind_draws_lines() {
    for kind in 1..KINDS.len() {
        let s = shape_from_drag(kind, &[[100.0, 100.0], [150.0, 140.0], [220.0, 130.0]], 8.0).unwrap();
        let l = lines(&s, 400.0, 300.0);
        assert!(!l.is_empty() && l.iter().all(|p| p.iter().flatten().all(|v| v.is_finite())), "{kind}");
    }
}
