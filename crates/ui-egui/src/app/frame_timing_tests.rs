//! Frame times with a 3D layer (run by hand: `cargo test --release -p photocraft-ui-egui --lib
//! frame_timing -- --ignored --nocapture`). Prints the average time of a frame for a photo-sized
//! document with and without a live 3D layer, and with the Object tool's panel and overlays.

use super::*;
use eframe::App as _;
use std::time::Instant;

fn frames(app: &mut PhotocraftApp, ctx: &egui::Context, n: usize, panel: bool) -> f64 {
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(2200.0, 1440.0));
    let run = |app: &mut PhotocraftApp| {
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let mut frame = eframe::Frame::_new_kittest();
        let mut out = ctx.run_ui(input, |ui| {
            app.logic(ui.ctx(), &mut frame);
            app.ui(ui, &mut frame);
            if panel {
                egui::Window::new("3D").show(ui.ctx(), |ui| crate::object_tool::materials(app, ui));
            }
        });
        out.textures_delta.clear();
    };
    for _ in 0..3 {
        run(app);
    }
    let t = Instant::now();
    for _ in 0..n {
        run(app);
    }
    t.elapsed().as_secs_f64() * 1000.0 / n as f64
}

#[test]
#[ignore]
fn frame_timing_with_a_3d_layer() {
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, crate::theme::ThemeKind::Pro);
    let mut app = PhotocraftApp::new(Session::new(), Services::default());
    app.session.execute("file.new", serde_json::json!({"width": 2500, "height": 3750})).unwrap();
    app.session.execute("edit.fill", serde_json::json!({"color": "#808070"})).unwrap();
    app.sync_views();
    let n = 40;
    eprintln!("photo only, Brush tool:            {:7.2} ms", frames(&mut app, &ctx, n, false));
    app.session.execute("layer.new.figure3d", serde_json::json!({"pose": "Standing"})).unwrap();
    app.sync_views();
    eprintln!("+ 3D layer, Brush tool:            {:7.2} ms", frames(&mut app, &ctx, n, false));
    app.ui.tool = crate::state::Tool::Object;
    eprintln!("+ Object tool:                     {:7.2} ms", frames(&mut app, &ctx, n, false));
    eprintln!("+ 3D panel (Poses tab):            {:7.2} ms", frames(&mut app, &ctx, n, true));
    app.ui.object_tool.tab = crate::object_tool::PanelTab::Photo;
    eprintln!("+ 3D panel (Match photo tab):      {:7.2} ms", frames(&mut app, &ctx, n, true));
    app.ui.object_tool.joints = true;
    eprintln!("+ Joints on:                       {:7.2} ms", frames(&mut app, &ctx, n, true));
    app.ui.object_tool.joints = false;
    app.ui.tool = crate::state::Tool::Brush;
    app.session.execute("layer.rasterize.scene3d", serde_json::json!({})).unwrap();
    app.sync_views();
    eprintln!("rasterized, Brush tool:            {:7.2} ms", frames(&mut app, &ctx, n, false));
}
