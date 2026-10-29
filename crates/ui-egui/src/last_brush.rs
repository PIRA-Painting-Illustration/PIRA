//! The brush the tool was last using, remembered across restarts (`Preferences::last_brush`) so
//! the next launch starts with the same brush, options bar and painting cursor outline.
//!
//! Saved once the pointer is up (a slider drag saves once, like the Brush Preset picker's view),
//! and change-driven: `remember_brush` watches `ToolState::brush_rev`, so an unchanged brush
//! costs a counter compare per frame, not a clone and a JSON serialisation of the whole brush.

use crate::PhotocraftApp;

/// Remember the current brush once the pointer is up.
pub fn persist(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if !ctx.input(|i| i.pointer.any_down()) {
        photocraft_engine::brush_cmds::remember_brush(&mut app.session);
    }
}

/// Restore the remembered brush at launch; stored tips load when the preset store attaches
/// (`Session::attach_preset_store`).
pub fn restore(app: &mut PhotocraftApp) {
    photocraft_engine::brush_cmds::restore_brush(&mut app.session);
}

/// A fresh install ([`crate::pira::first_launch`]) starts on PIRA's first brush, a pressure-
/// and tilt-ready pencil ([`photocraft_paint::presets::FIRST_BRUSH`]).
pub(crate) fn first_launch_brush(app: &mut PhotocraftApp) {
    let name = photocraft_paint::presets::FIRST_BRUSH;
    let tools = &mut app.session.tools;
    let Some(p) = photocraft_paint::presets::find(&tools.presets, name) else { return };
    let brush = photocraft_paint::BrushSettings { color: tools.brush.color, ..p.brush.clone() };
    tools.brush = brush;
    tools.current_preset = Some(name.to_string());
    tools.brush_changed();
}
