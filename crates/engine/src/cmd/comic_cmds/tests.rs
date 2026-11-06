use super::*;

/// A 1000 × 1400 comic page with 20 px bleed and 60 px safe margin, and a layout of `rows` ×
/// `columns` panels.
fn page() -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 1000, "height": 1400, "resolution": 350, "background": "white", "comicPage": {"bleed": 20, "safe": 60}})).unwrap();
    s
}

fn doc(s: &Session) -> &Document {
    &s.active().unwrap().doc
}

fn panels(s: &Session) -> Vec<Vec<[f64; 2]>> {
    doc(s).walk().into_iter().filter(|(_, _, l)| l.pira.panel).filter_map(|(_, _, l)| panel_outline(l)).collect()
}

#[test]
fn a_comic_page_keeps_its_trim_and_safe_area_with_guides() {
    let s = page();
    let d = doc(&s);
    assert_eq!(d.pira.page, Some(photocraft_doc::ComicPage { bleed: 20, safe: 60 }));
    assert_eq!(d.guides.vertical, vec![20.0, 980.0, 80.0, 920.0]);
    assert_eq!(d.guides.horizontal, vec![20.0, 1380.0, 80.0, 1320.0]);
    // It survives a save.
    let bytes = photocraft_format::save_to_bytes(d, &Default::default()).unwrap();
    let back = photocraft_format::load_from_bytes(&bytes).unwrap();
    assert_eq!(back.pira.page, d.pira.page);
    // Margins that don't fit are refused before any document opens.
    let mut s = Session::new();
    assert!(s.execute("file.new", json!({"width": 100, "height": 100, "comicPage": {"bleed": 20, "safe": 20}})).is_err());
    assert!(s.active().is_none());
}

#[test]
fn new_panels_fill_the_safe_area_in_a_grid() {
    let mut s = page();
    let r = s.execute(NEW_PANELS, json!({"rows": 2, "columns": 2, "gutter": 40, "border": 6})).unwrap();
    assert_eq!(r["panels"], 4);
    let p = panels(&s);
    assert_eq!(p.len(), 4);
    // The top-left panel starts at the safe area's corner.
    let first = p.iter().find(|o| o[0] == [80.0, 80.0]).expect("a panel at the safe area's corner");
    assert_eq!(first[2], [480.0, 680.0], "half the safe width less half the gutter");
    // One undo step.
    assert!(s.undo());
    assert!(panels(&s).is_empty());
    assert!(s.execute(NEW_PANELS, json!({"rows": 9})).is_err());
}

#[test]
fn panels_clip_their_art_and_draw_a_border() {
    let mut s = page();
    s.execute(NEW_PANELS, json!({"border": 6})).unwrap();
    // Paint the whole art layer red: only the panel shows it.
    let art = doc(&s).walk().into_iter().find(|(_, _, l)| l.name == "Panel 1 art").map(|(_, _, l)| l.id).unwrap();
    s.edit("paint", |doc, _| {
        doc.layer_mut(art).unwrap().surface_mut().unwrap().fill_rect(photocraft_geom::Rect::new(0, 0, 1000, 1400), &[1.0, 0.0, 0.0, 1.0]);
        Ok(())
    })
    .unwrap();
    let flat = crate::file_cmds::flattened(doc(&s), photocraft_color::PixelFormat::RGBA8);
    assert_eq!(flat.pixel(40, 40), vec![1.0, 1.0, 1.0, 1.0], "outside the panel: the page");
    assert_eq!(flat.pixel(500, 700), vec![1.0, 0.0, 0.0, 1.0], "inside: the art");
    assert_eq!(flat.pixel(82, 700)[0], 0.0, "the border, inside the outline");
}

#[test]
fn cutting_a_panel_leaves_two_a_gutter_apart() {
    let mut s = page();
    s.execute(NEW_PANELS, json!({})).unwrap();
    // A level cut across the middle of the one panel.
    let r = s.execute(CUT_PANEL, json!({"from": [0, 700], "to": [1000, 700], "gutter": 30})).unwrap();
    assert_eq!(r["panels"].as_array().unwrap().len(), 2);
    let p = panels(&s);
    assert_eq!(p.len(), 2);
    let ys: Vec<(f64, f64)> = p.iter().map(|o| (o.iter().map(|q| q[1]).fold(f64::MAX, f64::min), o.iter().map(|q| q[1]).fold(f64::MIN, f64::max))).collect();
    assert!(ys.contains(&(80.0, 685.0)) && ys.contains(&(715.0, 1320.0)), "{ys:?}");
    // A slanted cut through the lower panel.
    s.execute(CUT_PANEL, json!({"from": [300, 1400], "to": [700, 700]})).unwrap();
    assert_eq!(panels(&s).len(), 3);
    // A line that misses every panel is refused.
    assert!(s.execute(CUT_PANEL, json!({"from": [0, 10], "to": [1000, 10]})).is_err());
    assert!(s.undo());
    assert_eq!(panels(&s).len(), 2);
}

#[test]
fn split_handles_lines_that_miss() {
    let sq = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
    assert!(split(&sq, [0.0, 200.0], [100.0, 200.0], 10.0).is_none());
    let (a, b) = split(&sq, [50.0, -10.0], [50.0, 110.0], 10.0).unwrap();
    assert!((area(&a) - 4500.0).abs() < 1e-6 && (area(&b) - 4500.0).abs() < 1e-6);
}

#[test]
fn a_balloon_holds_its_words_and_points_its_tail() {
    let mut s = page();
    let r = s.execute(NEW_BALLOON, json!({"center": [500, 400], "size": [300, 200], "tail": [300, 700], "text": "Hey!"})).unwrap();
    let id = LayerId(r["layer"].as_u64().unwrap());
    let g = doc(&s).layer(id).unwrap();
    assert!(g.pira.balloon);
    let kids = g.children().unwrap();
    assert!(matches!(kids[0].content, LayerContent::Shape(_)) && matches!(kids[1].content, LayerContent::Text(_)), "the words over the shape");
    let flat = crate::file_cmds::flattened(doc(&s), photocraft_color::PixelFormat::RGBA8);
    assert_eq!(flat.pixel(500, 470)[0], 1.0, "white inside");
    // The tail reaches out to its tip.
    // Checked to be a shape above.
    let LayerContent::Shape(sh) = &kids[0].content else { return };
    let (x0, _, _, y1) = sh.path.control_bounds().unwrap();
    assert!(x0 <= 300.5 && y1 >= 699.5, "{x0} {y1}");
    for shape in ["rect", "cloud", "burst"] {
        s.execute(NEW_BALLOON, json!({"shape": shape, "tail": null})).unwrap();
    }
    assert!(s.execute(NEW_BALLOON, json!({"shape": "heart"})).is_err());
    assert!(s.execute(NEW_BALLOON, json!({"size": [4, 4]})).is_err());
}

#[test]
fn focus_lines_leave_the_middle_clear_and_speed_lines_run_across() {
    let mut s = page();
    s.execute(EFFECT_LINES, json!({"kind": "focus", "center": [500, 700], "inner": [200, 200], "count": 200})).unwrap();
    let flat = crate::file_cmds::flattened(doc(&s), photocraft_color::PixelFormat::RGBA8);
    assert_eq!(flat.pixel(500, 700)[0], 1.0, "the middle stays clear");
    let ink = (0..1000).filter(|x| flat.pixel(*x, 30)[0] < 0.5).count();
    assert!(ink > 20, "lines reach the edges: {ink}");
    s.execute(EFFECT_LINES, json!({"kind": "speed", "angle": 0, "count": 40})).unwrap();
    assert_eq!(doc(&s).walk().iter().filter(|(_, _, l)| matches!(l.content, LayerContent::Shape(_))).count(), 2);
    assert!(s.execute(EFFECT_LINES, json!({"kind": "zoom"})).is_err());
}

#[test]
fn screentone_is_a_dot_pattern_at_its_density() {
    let (size, px) = tone_tile(350.0, 60.0, 0.3);
    let ink = px.chunks(4).map(|p| f64::from(p[3])).sum::<f64>() / f64::from(size * size);
    assert!((ink - 0.3).abs() < 0.03, "{ink}");
    let mut s = page();
    let r = s.execute(NEW_TONE, json!({"density": 30, "lpi": 60})).unwrap();
    let id = LayerId(r["layer"].as_u64().unwrap());
    assert!(matches!(doc(&s).layer(id).unwrap().content, LayerContent::Fill(photocraft_doc::Fill::Pattern { .. })));
    assert!(doc(&s).patterns.iter().any(|p| p.id == r["pattern"]));
    assert!(s.execute(NEW_TONE, json!({"density": 95})).is_err());
}
