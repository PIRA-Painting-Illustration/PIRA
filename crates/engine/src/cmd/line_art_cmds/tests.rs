use super::*;
use photocraft_doc::LayerId;

fn px(s: &Session, x: i32, y: i32) -> Vec<f32> {
    let d = s.active().unwrap();
    d.doc.layer(d.active_layer.unwrap()).unwrap().surface().unwrap().pixel(x, y)
}

/// White Background with a black line at x = 10 (with a `gap`-pixel break from y = 4), a second
/// layer with its own line at x = 5, and an empty active layer on top to fill.
fn page(gap: i32) -> (Session, LayerId, LayerId) {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 20, "height": 10, "background": "white"})).unwrap();
    let ink = s.active().unwrap().active_layer.unwrap();
    s.edit("line", |doc, _| {
        let surf = doc.layer_mut(ink).unwrap().surface_mut().unwrap();
        surf.fill_rect(Rect::new(10, 0, 11, 4), &[0.0, 0.0, 0.0, 1.0]);
        surf.fill_rect(Rect::new(10, 4 + gap, 11, 10), &[0.0, 0.0, 0.0, 1.0]);
        Ok(())
    })
    .unwrap();
    s.execute("layer.new.layer", json!({})).unwrap();
    let sketch = s.active().unwrap().active_layer.unwrap();
    s.edit("sketch", |doc, _| {
        doc.layer_mut(sketch).unwrap().surface_mut().unwrap().fill_rect(Rect::new(5, 0, 6, 10), &[0.0, 0.0, 1.0, 1.0]);
        Ok(())
    })
    .unwrap();
    s.execute("layer.new.layer", json!({})).unwrap();
    (s, ink, sketch)
}

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

fn bucket(s: &mut Session, extra: Value) -> Value {
    let mut p = json!({"x": 2, "y": 2, "color": "#ff0000", "antiAlias": false, "tolerance": 0});
    p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    s.execute("paint.bucket", p).unwrap()
}

#[test]
fn the_bucket_finds_its_area_on_the_reference_layers_only() {
    let (mut s, ink, _) = page(0);
    let fill = s.active().unwrap().active_layer.unwrap();
    s.execute(SET_REFERENCE, json!({"layer": ink.0})).unwrap();
    assert_eq!(s.active().unwrap().active_layer, Some(fill), "marking a layer leaves the active layer alone");
    bucket(&mut s, json!({"sample": "reference"}));
    assert_eq!(px(&s, 7, 5), RED, "the sketch line isn't a reference: the fill crosses it");
    assert_eq!(px(&s, 15, 5), vec![0.0; 4], "the ink line holds the fill");
    assert!(s.undo());
    bucket(&mut s, json!({"sample": "all"}));
    assert_eq!(px(&s, 7, 5), vec![0.0; 4], "all layers: the sketch line holds it");
}

#[test]
fn reference_needs_a_reference_layer_and_bad_values_are_refused() {
    let (mut s, _, _) = page(0);
    let p = json!({"x": 2, "y": 2, "sample": "reference"});
    let e = s.execute("paint.bucket", p).unwrap_err().to_string();
    assert!(e.contains("Reference Layer"), "{e}");
    assert!(s.execute("paint.bucket", json!({"x": 2, "y": 2, "sample": "everything"})).is_err());
    assert!(s.execute("paint.bucket", json!({"x": 2, "y": 2, "closeGap": 500})).is_err());
    assert!(s.execute("select.magicWand", json!({"x": 2, "y": 2, "expand": -1})).is_err());
    assert_eq!(px(&s, 2, 2), vec![0.0; 4], "nothing was filled");
}

#[test]
fn reference_toggles_as_one_undo_step_and_shows_in_the_layer_info() {
    let (mut s, ink, _) = page(0);
    let past = s.active().unwrap().history.past_len();
    let r = s.execute(SET_REFERENCE, json!({"layer": ink.0})).unwrap();
    assert_eq!(r["reference"], true);
    assert!(s.active().unwrap().doc.layer(ink).unwrap().pira.reference);
    assert_eq!(s.active().unwrap().history.past_len(), past + 1);
    assert_eq!(s.execute(SET_REFERENCE, json!({"layer": ink.0})).unwrap()["reference"], false, "toggles");
    s.execute(SET_REFERENCE, json!({"layer": ink.0, "reference": false})).unwrap();
    assert_eq!(s.active().unwrap().history.past_len(), past + 2, "no change, no step");
    assert!(s.undo());
    assert!(s.active().unwrap().doc.layer(ink).unwrap().pira.reference);
}

#[test]
fn a_hidden_reference_layer_doesnt_count() {
    let (mut s, ink, _) = page(0);
    s.execute(SET_REFERENCE, json!({"layer": ink.0})).unwrap();
    s.edit("hide", |doc, _| {
        doc.layer_mut(ink).unwrap().visible = false;
        Ok(())
    })
    .unwrap();
    assert!(reference_only(&s.active().unwrap().doc).is_none());
}

#[test]
fn close_gap_holds_the_fill_behind_a_broken_line() {
    let (mut s, _, sketch) = page(2);
    // Only the ink line: hide the sketch.
    s.edit("hide", |doc, _| {
        doc.layer_mut(sketch).unwrap().visible = false;
        Ok(())
    })
    .unwrap();
    bucket(&mut s, json!({"sample": "all", "x": 3}));
    assert_eq!(px(&s, 15, 5), RED, "without Close Gap the fill leaks through");
    assert!(s.undo());
    bucket(&mut s, json!({"sample": "all", "x": 3, "closeGap": 3}));
    assert_eq!(px(&s, 15, 5), vec![0.0; 4], "Close Gap holds it");
    assert_eq!(px(&s, 9, 0), RED, "and still fills up to the line");
    assert_eq!(px(&s, 10, 0), vec![0.0; 4], "but not over it");
    assert!(s.undo());
    bucket(&mut s, json!({"sample": "all", "x": 3, "closeGap": 3, "expand": 1}));
    assert_eq!(px(&s, 10, 0), RED, "Expand reaches under the line");
}

#[test]
fn close_gap_works_on_the_layer_itself() {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 20, "height": 10, "background": "white"})).unwrap();
    let ink = s.active().unwrap().active_layer.unwrap();
    s.edit("line", |doc, _| {
        let surf = doc.layer_mut(ink).unwrap().surface_mut().unwrap();
        surf.fill_rect(Rect::new(10, 0, 11, 4), &[0.0, 0.0, 0.0, 1.0]);
        surf.fill_rect(Rect::new(10, 6, 11, 10), &[0.0, 0.0, 0.0, 1.0]);
        Ok(())
    })
    .unwrap();
    bucket(&mut s, json!({"x": 3, "closeGap": 3}));
    assert_eq!(px(&s, 3, 3), RED);
    assert_eq!(px(&s, 15, 5), vec![1.0; 4], "the white beyond the line stays white");
}

/// A lasso around the left half (x 1..9, y 1..9) of the page.
fn left_lasso() -> Value {
    json!([[1, 1], [9, 1], [9, 9], [1, 9]])
}

#[test]
fn lasso_fill_fills_the_shape_drawn() {
    let (mut s, _, _) = page(0);
    let r = s.execute(LASSO_FILL, json!({"points": left_lasso(), "color": "#ff0000", "antiAlias": false})).unwrap();
    assert_eq!(r["filled"], true);
    assert_eq!(px(&s, 3, 3), RED);
    assert_eq!(px(&s, 0, 0), vec![0.0; 4], "outside the lasso stays empty");
    assert!(s.execute(LASSO_FILL, json!({"points": [[1, 1], [2, 2]]})).is_err(), "two points aren't a lasso");
    assert!(s.execute(LASSO_FILL, json!({"points": left_lasso(), "kind": "splash"})).is_err());
}

#[test]
fn lasso_fill_enclosed_fills_only_areas_wholly_inside() {
    let (mut s, _, _) = page(0);
    // The sketch line at x = 5 splits the canvas: x 0..4 and x 6..9 (up to the ink at 10). The
    // lasso covers x 6..9 but only part of x 0..4, so only the right-hand area is enclosed.
    let lasso = json!([[3, -1], [10.5, -1], [10.5, 11], [3, 11]]);
    s.execute(LASSO_FILL, json!({"points": lasso, "kind": "enclosed", "color": "#ff0000"})).unwrap();
    assert_eq!(px(&s, 7, 5), RED, "between the lines, wholly inside");
    assert_eq!(px(&s, 4, 5), vec![0.0; 4], "the left area crosses the lasso: not filled");
    assert_eq!(px(&s, 5, 5), vec![0.0; 4], "the line itself isn't filled");
    assert!(s.undo());
    s.execute(LASSO_FILL, json!({"points": lasso, "kind": "enclosed", "color": "#ff0000", "expand": 1})).unwrap();
    assert_eq!(px(&s, 5, 5), RED, "Expand goes under the line");
}

#[test]
fn lasso_fill_unfilled_only_touches_empty_pixels() {
    let (mut s, _, _) = page(0);
    s.edit("blue spot", |doc, active| {
        doc.layer_mut(active.unwrap()).unwrap().surface_mut().unwrap().fill_rect(Rect::new(2, 2, 4, 4), &[0.0, 0.0, 1.0, 1.0]);
        Ok(())
    })
    .unwrap();
    s.execute(LASSO_FILL, json!({"points": left_lasso(), "kind": "unfilled", "color": "#ff0000", "antiAlias": false})).unwrap();
    assert_eq!(px(&s, 3, 3), vec![0.0, 0.0, 1.0, 1.0], "the painted spot stays");
    assert_eq!(px(&s, 6, 6), RED, "the gaps around it fill");
}

#[test]
fn draft_layers_stay_out_of_exports_and_fills() {
    let (mut s, _, sketch) = page(0);
    s.execute(SET_DRAFT, json!({"layer": sketch.0})).unwrap();
    assert!(s.active().unwrap().doc.layer(sketch).unwrap().pira.draft);
    // A PNG leaves the draft out; it's still on the canvas.
    let doc = s.active().unwrap().doc.clone();
    let (png, _) = crate::file_cmds::encode(&doc, "x.png", None::<f64>).unwrap();
    let back = photocraft_io::import("x.png", &png).unwrap().document;
    let flat = crate::file_cmds::flattened(&back, photocraft_color::PixelFormat::RGBA8);
    assert_eq!(flat.pixel(5, 8)[0], 1.0, "the white page shows where the draft line was");
    assert!(s.active().unwrap().doc.layer(sketch).unwrap().visible, "the draft still shows in PIRA");
    // The sketch line at x = 5 doesn't bound an all-layers fill any more.
    bucket(&mut s, json!({"sample": "all"}));
    assert_eq!(px(&s, 7, 5), RED);
}

#[test]
fn line_color_toggles_a_color_overlay_and_undoes() {
    let (mut s, _, sketch) = page(0);
    let r = s.execute(SET_LINE_COLOR, json!({"layer": sketch.0})).unwrap();
    assert_eq!(r["color"], "#3a7bd5");
    let fx = |s: &Session| s.active().unwrap().doc.layer(sketch).unwrap().effects.items.len();
    assert_eq!(fx(&s), 1, "one Color Overlay");
    s.execute(SET_LINE_COLOR, json!({"layer": sketch.0, "color": "#ff0000"})).unwrap();
    assert_eq!(fx(&s), 1, "replaced, not added");
    assert_eq!(s.active().unwrap().doc.layer(sketch).unwrap().pira.line_color, Some([255, 0, 0]));
    assert_eq!(s.execute(SET_LINE_COLOR, json!({"layer": sketch.0})).unwrap()["color"], Value::Null, "toggles off");
    assert_eq!(fx(&s), 0);
    assert!(s.undo());
    assert_eq!(fx(&s), 1);
    assert!(s.execute(SET_LINE_COLOR, json!({"layer": sketch.0, "color": "blue"})).is_err());
}

#[test]
fn border_effect_outlines_the_layer_and_keeps_its_stroke_in_step() {
    let (mut s, _, sketch) = page(0);
    let stroke = |s: &Session| {
        let l = s.active().unwrap().doc.layer(sketch).unwrap();
        l.effects.items.iter().filter_map(|e| if let photocraft_doc::Effect::Stroke(st) = e { Some(st.size) } else { None }).collect::<Vec<_>>()
    };
    let r = s.execute(SET_BORDER, json!({"layer": sketch.0})).unwrap();
    assert_eq!((r["width"].clone(), r["color"].clone()), (json!(4), json!("#ffffff")));
    assert_eq!(stroke(&s), vec![4.0]);
    s.execute(SET_BORDER, json!({"layer": sketch.0, "width": 9})).unwrap();
    assert_eq!(stroke(&s), vec![9.0], "resized, not added");
    s.execute(SET_BORDER, json!({"layer": sketch.0, "color": "#000000"})).unwrap();
    assert_eq!(
        s.active().unwrap().doc.layer(sketch).unwrap().pira.border,
        Some(photocraft_doc::Border { width: 9, color: [0, 0, 0] }),
        "a colour alone keeps the width"
    );
    // The outline shows on the canvas: black beside the blue line, where there was nothing.
    let flat = crate::file_cmds::flattened(&s.active().unwrap().doc, photocraft_color::PixelFormat::RGBA8);
    assert_eq!(flat.pixel(7, 5)[0], 0.0, "the border is drawn");
    assert!(s.execute(SET_BORDER, json!({"layer": sketch.0, "width": 0})).is_err());
    s.execute(SET_BORDER, json!({"layer": sketch.0, "width": null})).unwrap();
    assert!(stroke(&s).is_empty());
}
