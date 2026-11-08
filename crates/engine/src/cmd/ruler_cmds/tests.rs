use super::*;

fn close(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6
}

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 1000, "height": 800})).unwrap();
    s
}

fn shapes(s: &Session) -> Vec<RulerShape> {
    s.active().unwrap().rulers_here().into_iter().map(|(_, r)| r.shape.clone()).collect()
}

#[test]
fn a_straight_ruler_takes_strokes_near_it_only() {
    let r = RulerShape::Straight { a: [100.0, 100.0], b: [900.0, 100.0] };
    let g = guide(&r, [300.0, 120.0], 40.0).unwrap();
    assert!(close(g.project([500.0, 140.0]), [500.0, 100.0]), "along the ruler itself");
    assert!(guide(&r, [300.0, 300.0], 40.0).is_none(), "too far away: draws freely");
}

#[test]
fn parallel_and_radial_rulers_apply_from_wherever_a_stroke_starts() {
    let par = RulerShape::Parallel { a: [0.0, 0.0], b: [100.0, 100.0] };
    let g = guide(&par, [500.0, 200.0], 0.0).unwrap();
    let q = g.project([600.0, 280.0]);
    assert!((q[0] - 500.0 - (q[1] - 200.0)).abs() < 1e-9, "45Â°, through the start: {q:?}");
    let rad = RulerShape::Radial { center: [500.0, 400.0] };
    let g = guide(&rad, [100.0, 400.0], 0.0).unwrap();
    assert!(close(g.project([300.0, 450.0]), [300.0, 400.0]), "towards the centre");
    assert!(guide(&rad, [500.0, 400.0], 0.0).is_none(), "no direction at the centre");
}

#[test]
fn a_concentric_ruler_runs_round_the_ellipse_through_the_start() {
    let r = RulerShape::Concentric { center: [500.0, 400.0], radii: [200.0, 100.0], angle: 0.0 };
    // Starting at (900, 400): the ellipse twice the ruler's size.
    let g = guide(&r, [900.0, 400.0], 0.0).unwrap();
    assert!(close(g.project([500.0, 100.0]), [500.0, 200.0]), "the top of the 400 Ã— 200 ellipse");
    let t = g.tangent([900.0, 400.0]).unwrap();
    assert!(t[0].abs() < 1e-9, "upright at its right end: {t:?}");
    // A circle, turned: still a circle.
    let c = RulerShape::Concentric { center: [0.0, 0.0], radii: [50.0, 50.0], angle: 1.0 };
    let g = guide(&c, [30.0, 40.0], 0.0).unwrap();
    let q = g.project([0.0, -200.0]);
    assert!(close(q, [0.0, -50.0]), "{q:?}");
}

#[test]
fn a_fisheye_ruler_bends_lines_through_its_vanishing_points() {
    let r = RulerShape::Fisheye { a: [0.0, 400.0], b: [1000.0, 400.0] };
    let g = guide(&r, [500.0, 200.0], 0.0).unwrap();
    assert!(matches!(g, Guide::Circle { .. }), "{g:?}");
    let Guide::Circle { center, r: radius } = g else { return };
    for p in [[0.0, 400.0], [1000.0, 400.0], [500.0, 200.0]] {
        assert!(((p[0] - center[0]).hypot(p[1] - center[1]) - radius).abs() < 1e-6, "{p:?} on the arc");
    }
    // On the eye level itself: the straight eye level.
    assert!(matches!(guide(&r, [500.0, 400.0], 0.0), Some(Guide::Line { .. })));
}

#[test]
fn a_curve_ruler_passes_through_its_points() {
    let pts = vec![[0.0, 0.0], [100.0, 50.0], [200.0, 0.0]];
    let path = curve_path(&pts);
    for p in &pts {
        assert!(path.iter().any(|q| close(*q, *p)), "{p:?} on the curve");
    }
    let r = RulerShape::Curve { points: pts };
    let g = guide(&r, [100.0, 60.0], 20.0).unwrap();
    assert!(g.distance([100.0, 50.0]) < 1e-6);
}

#[test]
fn a_stroke_picks_the_guide_it_heads_along() {
    let rulers = [RulerShape::Parallel { a: [0.0, 0.0], b: [1.0, 0.0] }, RulerShape::Radial { center: [500.0, 0.0] }];
    // Heading level: the parallel ruler.
    let g = pick(&rulers, [500.0, 400.0], [560.0, 410.0], 0.0).unwrap();
    assert_eq!(g, Guide::Line { origin: [500.0, 400.0], dir: [1.0, 0.0] });
    // Heading up: the radial one (straight up to its centre).
    let g = pick(&rulers, [500.0, 400.0], [505.0, 340.0], 0.0).unwrap();
    assert!(matches!(g, Guide::Line { dir, .. } if dir[0].abs() < 1e-9));
    assert!(pick(&rulers, [1.0, 1.0], [1.0, 1.0], 0.0).is_none(), "not moved yet");
}

#[test]
fn rulers_belong_to_their_layer_undo_and_survive_a_save() {
    let mut s = session();
    assert!(s.execute("ruler.add", json!({"type": "straight", "a": [0, 0], "b": [0, 0]})).is_err(), "two distinct points");
    assert!(s.execute("ruler.add", json!({"type": "spiral"})).is_err());
    assert!(s.execute("ruler.add", json!({"type": "curve", "points": [[0, 0]]})).is_err());
    let r = s.execute("ruler.add", json!({"type": "parallel", "a": [0, 0], "b": [10, 0]})).unwrap();
    assert_eq!(r["index"], 0);
    s.execute("ruler.add", json!({"type": "radial", "center": [500, 400], "allLayers": true})).unwrap();
    assert_eq!(shapes(&s).len(), 2);
    // A new layer: only the every-layer ruler shows there.
    s.execute("layer.new.layer", json!({})).unwrap();
    assert_eq!(shapes(&s), vec![RulerShape::Radial { center: [500.0, 400.0] }]);
    // Clear deletes just what shows here.
    s.execute("ruler.clear", json!({})).unwrap();
    assert_eq!(s.active().unwrap().doc.pira.rulers.len(), 1);
    assert!(s.undo());
    assert_eq!(s.active().unwrap().doc.pira.rulers.len(), 2);
    // Edit and delete by index.
    s.execute("ruler.set", json!({"index": 1, "shape": {"type": "concentric", "center": [500, 400], "radii": [100, 50], "angle": 0}})).unwrap();
    assert_eq!(s.active().unwrap().doc.pira.rulers[1].shape.kind(), "concentric");
    assert!(s.execute("ruler.set", json!({"index": 5})).is_err());
    let doc = &s.active().unwrap().doc;
    let bytes = photocraft_format::save_to_bytes(doc, &Default::default()).unwrap();
    let back = photocraft_format::load_from_bytes(&bytes).unwrap();
    assert_eq!(back.pira.rulers, doc.pira.rulers);
    s.execute("ruler.delete", json!({"index": 0})).unwrap();
    assert_eq!(s.active().unwrap().doc.pira.rulers.len(), 1);
    // Snap and Show are switches, not history.
    s.execute("ruler.snap", json!({})).unwrap();
    let l = s.execute("ruler.list", json!({})).unwrap();
    assert_eq!(l["snap"], false);
    assert_eq!(l["rulers"][0]["type"], "concentric");
}
