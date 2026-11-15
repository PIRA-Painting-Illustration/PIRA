//! #1810: File › New starts from the last document made with the dialog (Photoshop does), with the
//! clipboard image's size still taking priority.

use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::state::DialogKind;

fn app() -> PhotocraftApp {
    PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default())
}

/// Open New Document, set `fields`, press Create.
fn create(app: &mut PhotocraftApp, fields: Value) {
    let f = app.new_document_fields();
    let id = app.ui.open_dialog(DialogKind::NewDocument, f);
    let d = app.ui.dialog_mut(id).unwrap();
    for (k, v) in fields.as_object().unwrap() {
        d.fields.insert(k.clone(), v.clone());
    }
    crate::dialogs::confirm(app, id).unwrap();
}

#[test]
fn new_document_starts_from_the_last_settings() {
    let mut app = app();
    // The first time: the defaults (1920 × 1080, 72 ppi).
    assert_eq!(app.new_document_fields(), crate::state::UiState::new_document_fields());
    create(
        &mut app,
        json!({"name": "Poster", "width": 640, "height": 480, "resolution": 300.0, "mode": "cmyk", "depth": 16, "background": "transparent", "__unit": "in", "__resUnit": "ppi"}),
    );
    let d = &app.session.active().unwrap().doc;
    assert_eq!((d.size.width, d.size.height, d.resolution_dpi), (640, 480, 300.0));
    // The next time: those settings, but not the name.
    let f = app.new_document_fields();
    for (k, v) in [
        ("width", json!(640)),
        ("height", json!(480)),
        ("resolution", json!(300.0)),
        ("mode", json!("cmyk")),
        ("depth", json!(16)),
        ("background", json!("transparent")),
        ("__unit", json!("in")),
        ("__resUnit", json!("ppi")),
    ] {
        assert_eq!(f.get(k), Some(&v), "{k}");
    }
    assert_eq!(f["name"], "Untitled-1");
    // They live in the preferences, so they survive a restart.
    assert_eq!(app.session.prefs().dialogs["file.new"]["resolution"], json!(300.0));

    // An image on the clipboard still sets the size (the Clipboard preset); the rest is remembered.
    app.run("select.rect", json!({"x": 0, "y": 0, "width": 123, "height": 45})).unwrap();
    app.run("edit.fill", json!({"color": "#336699"})).unwrap();
    app.run("edit.copy", json!({})).unwrap();
    let f = app.new_document_fields();
    assert_eq!((f["width"].as_u64(), f["height"].as_u64()), (Some(123), Some(45)));
    assert_eq!((f["mode"].as_str(), f["depth"].as_u64()), (Some("cmyk"), Some(16)));
}

#[test]
fn a_corrupt_remembered_value_is_ignored() {
    let mut app = app();
    app.session.prefs.edit(|p| p.dialogs.insert("file.new".into(), json!({"width": -5, "height": 600, "resolution": "x", "depth": 7, "mode": 3})));
    let f = app.new_document_fields();
    let defaults = crate::state::UiState::new_document_fields();
    assert_eq!(f["height"], json!(600), "a valid value is used");
    for k in ["width", "depth", "mode"] {
        assert_eq!(f.get(k), defaults.get(k), "{k}");
    }
    assert!(f.get("resolution").is_none());
    // A failed Create remembers nothing.
    let id = app.ui.open_dialog(DialogKind::NewDocument, f);
    app.ui.dialog_mut(id).unwrap().fields.insert("width".into(), json!(0));
    let _ = crate::dialogs::confirm(&mut app, id);
    assert_eq!(app.session.prefs().dialogs["file.new"]["height"], json!(600), "unchanged");
}

/// #3131: File › New › Recent lists the documents created with the dialog, newest first, with
/// every setting restored by a click, and survives a restart.
#[test]
fn created_documents_fill_the_recent_category() {
    let mut app = app();
    create(&mut app, json!({"width": 640, "height": 480, "resolution": 300.0, "mode": "cmyk", "depth": 16, "background": "transparent", "__unit": "in"}));
    create(&mut app, json!({"width": 1920, "height": 1080, "resolution": 72.0, "mode": "rgb", "depth": 8, "background": "white", "__unit": "px"}));
    let recent = app.session.presets.recent_documents.clone();
    assert_eq!(recent.iter().map(|r| (r.width, r.unit.as_str())).collect::<Vec<_>>(), [(1920, "px"), (640, "in")]);
    // The settings are kept with the preferences.
    let mut restarted = photocraft_engine::Session::new();
    restarted.load_prefs_json(&app.session.prefs_to_json()).unwrap();
    assert_eq!(restarted.presets.recent_documents, recent);
    // A recent card restores all of its settings but keeps the new document's name.
    let mut f = app.new_document_fields();
    f.insert("name".into(), json!("Next"));
    crate::new_doc_ui::apply_settings(&mut f, &recent[1]);
    let p = crate::new_doc_ui::command_params(&f);
    assert_eq!(
        (&p["width"], &p["height"], &p["mode"], &p["depth"], &p["background"], &p["name"], &f["__unit"]),
        (&json!(640), &json!(480), &json!("cmyk"), &json!(16), &json!("transparent"), &json!("Next"), &json!("in"))
    );
}

/// A new canvas starts on the Pencil, whatever tool the last one ended on; a document opened or
/// switched to keeps the tool in use.
#[test]
fn a_new_document_starts_on_the_pencil_and_others_keep_the_tool() {
    use crate::drawing_kit::{Kit, active, switch};
    let mut app = app();
    app.ui.tool = crate::state::Tool::Move;
    create(&mut app, json!({"width": 64, "height": 64}));
    assert_eq!(active(&app), Some(Kit::Pencil));
    // On the Pen, then another document is made the active one (as opening one does).
    switch(&mut app, Kit::Pen);
    app.session.execute("file.new", json!({"width": 32, "height": 32})).unwrap();
    assert_eq!(active(&app), Some(Kit::Pen), "only the New dialog picks the Pencil");
    create(&mut app, json!({"width": 64, "height": 64}));
    assert_eq!(active(&app), Some(Kit::Pencil));
}

#[test]
fn a_fresh_install_new_document_opens_on_an_active_layer_1() {
    let mut app = app();
    crate::pira::first_launch(&mut app);
    create(&mut app, json!({"width": 5400, "height": 7200, "resolution": 300.0, "timelapse": true, "__preset": "Poster"}));
    // A frame, as the tablet draws one before the first stroke.
    let ctx = egui::Context::default();
    for _ in 0..2 {
        use eframe::App as _;
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(2200.0, 1440.0))), ..Default::default() };
        let mut frame = eframe::Frame::_new_kittest();
        ctx.run_ui(input, |ui| {
            app.logic(ui.ctx(), &mut frame);
            app.ui(ui, &mut frame);
        })
        .textures_delta
        .clear();
    }
    let st = app.session.active().unwrap();
    let top = st.doc.layers.last().unwrap();
    assert_eq!(top.name, "Layer 1");
    assert_eq!(st.active_layer, Some(top.id));
    assert_eq!(st.selected_layers(), vec![top.id]);
}

/// On the tablet: the whole app, PIRA's layout, the New dialog's Create tapped with a finger
/// (the pen taps the same way). Layer 1 stays the selected layer afterwards.
#[test]
fn tapping_create_on_the_tablet_leaves_layer_1_selected() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;
    let mut app = app();
    crate::pira::first_launch(&mut app);
    let mut h = Harness::builder().with_size(egui::vec2(2200.0, 1440.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, app: &mut PhotocraftApp| {
            use eframe::App as _;
            let mut frame = eframe::Frame::_new_kittest();
            app.logic(ui.ctx(), &mut frame);
            app.ui(ui, &mut frame);
        },
        app,
    );
    PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::Pira);
    h.run_steps(4);
    // Android's saved tool state loads after `first_launch`: a fresh one is the Move tool.
    h.state_mut().ui.tool = crate::state::Tool::Move;
    let f = h.state_mut().new_document_fields();
    h.state_mut().ui.open_dialog(DialogKind::NewDocument, f);
    h.run_steps(10);
    for (dx, dy) in [(0.0, 0.0), (-30.0, 0.0), (30.0, 0.0)] {
        let at = h.get_by_label("Create").rect().center() + egui::vec2(dx, dy);
        let touch = |phase| egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(1), phase, pos: at, force: None };
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        h.input_mut().events.extend([egui::Event::PointerMoved(at), touch(egui::TouchPhase::Start), button(true)]);
        h.step();
        h.input_mut().events.extend([touch(egui::TouchPhase::End), button(false)]);
        h.step();
        h.input_mut().events.push(egui::Event::PointerGone);
        h.run_steps(4);
        let st = h.state().session.active().expect("Create made the document");
        let top = st.doc.layers.last().unwrap();
        assert_eq!(top.name, "Layer 1");
        assert_eq!(st.active_layer, Some(top.id), "tap at {at:?}");
        assert_eq!(st.selected_layers(), vec![top.id], "tap at {at:?}");
        let f = h.state_mut().new_document_fields();
        h.state_mut().ui.open_dialog(DialogKind::NewDocument, f);
        h.run_steps(10);
    }
}
