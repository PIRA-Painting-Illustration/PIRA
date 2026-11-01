//! PIRA's three drawing tools on the touch bar, split the way Clip Studio Paint splits them: a
//! Pen for line art, a Pencil for sketching and shading, and a Brush for painting. Each is a
//! family of PIRA's built-in presets (`presets::PENS`, `PENCILS`, `BRUSHES`) and remembers the
//! brush it was last using, so one tap goes from inking back to the exact pencil you sketched
//! with.
//!
//! They sit on top of the Photoshop tools rather than replacing them: the Pen and the Pencil
//! paint with the Brush Tool (the textured pencils, not Photoshop's jagged Pencil Tool, which
//! stays in the full tool list for pixel art), and the Brush paints with the Brush Tool or, for
//! the brushes that mix with the canvas (`presets::MIXING`), the Mixer Brush.

use photocraft_engine::BrushSettings;
use photocraft_paint::presets;
use serde::{Deserialize, Serialize};

use crate::PhotocraftApp;
use crate::state::Tool;

/// One of the three drawing tools.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kit {
    Pen,
    #[default]
    Pencil,
    Brush,
}

impl Kit {
    pub const ALL: [Kit; 3] = [Kit::Pen, Kit::Pencil, Kit::Brush];

    pub fn label(self) -> &'static str {
        match self {
            Kit::Pen => "Pen",
            Kit::Pencil => "Pencil",
            Kit::Brush => "Brush",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Kit::Pen => "pen-tool",
            Kit::Pencil => "pencil",
            Kit::Brush => "brush",
        }
    }

    /// The preset group this tool's brushes come from.
    pub fn group(self) -> &'static str {
        match self {
            Kit::Pen => presets::PENS,
            Kit::Pencil => presets::PENCILS,
            Kit::Brush => presets::BRUSHES,
        }
    }

    /// The brush the tool starts with before it has been used.
    pub fn first_preset(self) -> &'static str {
        match self {
            Kit::Pen => "G-Pen",
            Kit::Pencil => presets::FIRST_BRUSH,
            Kit::Brush => "Watercolour",
        }
    }

    /// The tool whose preset group holds `group`, if it is one of PIRA's families.
    pub fn of_group(group: &str) -> Option<Kit> {
        Kit::ALL.into_iter().find(|k| k.group() == group)
    }
}

/// A drawing tool's remembered brush and the preset it came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KitBrush {
    pub kit: Kit,
    pub brush: BrushSettings,
    pub preset: Option<String>,
}

/// The tool a preset paints with: the Mixer Brush for the mixing brushes, else the Brush Tool.
fn tool_for(preset: Option<&str>) -> Tool {
    if preset.is_some_and(presets::mixes) { Tool::MixerBrush } else { Tool::Brush }
}

/// The drawing tool in use, if the current tool is one the three cover.
pub fn active(app: &PhotocraftApp) -> Option<Kit> {
    matches!(app.ui.tool, Tool::Brush | Tool::MixerBrush).then_some(app.ui.kit)
}

/// Switch to `kit`: the current brush is remembered for the tool being left, and `kit` picks up
/// where it was last left (or starts on its first brush), on the Brush Tool or the Mixer Brush
/// as that brush needs. The colours stay the session's.
pub fn switch(app: &mut PhotocraftApp, kit: Kit) {
    if active(app) == Some(kit) {
        return;
    }
    if active(app).is_some() {
        remember(app);
    }
    let saved = app.ui.kit_brushes.iter().find(|k| k.kit == kit).cloned();
    let (brush, preset) = match saved {
        Some(k) => (k.brush, k.preset),
        None => match presets::find(&app.session.tools.presets, kit.first_preset()) {
            Some(p) => (p.brush.clone(), Some(p.name.clone())),
            None => (app.session.tools.brush.clone(), None),
        },
    };
    app.ui.kit = kit;
    use_brush(app, brush, preset);
}

/// Remember the current brush for the drawing tool in use. A tool that has never had a brush of
/// its own doesn't take a stray one with no preset (the brush a session happened to start with):
/// it starts on its first brush instead.
fn remember(app: &mut PhotocraftApp) {
    let entry = KitBrush { kit: app.ui.kit, brush: app.session.tools.brush.clone(), preset: app.session.tools.current_preset.clone() };
    match app.ui.kit_brushes.iter_mut().find(|k| k.kit == entry.kit) {
        Some(k) => *k = entry,
        None if entry.preset.is_some() => app.ui.kit_brushes.push(entry),
        None => {}
    }
}

/// Paint with `brush` (from `preset`) on the tool it needs, keeping the session's colours, erase
/// flag and stroke seed.
fn use_brush(app: &mut PhotocraftApp, brush: BrushSettings, preset: Option<String>) {
    app.ui.tool = tool_for(preset.as_deref());
    // Let the per-tool brush memory swap first, then put this brush over whatever it loaded.
    crate::paint_mouse::sync_tool_brush(app);
    let current = &app.session.tools.brush;
    let mut b = BrushSettings { color: current.color, background: current.background, erase: current.erase, seed: current.seed, ..brush };
    let _ = app.session.load_brush_tips(&mut b);
    app.session.tools.brush = b;
    app.session.tools.current_preset = preset;
    app.session.tools.brush_changed();
}

/// Keep the drawing tools in step with a brush picked anywhere (the picker, the Brushes panel):
/// a PIRA preset makes its family the tool in use, and a mixing brush paints with the Mixer
/// Brush (any other with the Brush Tool). Cheap when nothing changed; run once a frame.
pub fn follow_preset(app: &mut PhotocraftApp) {
    if !matches!(app.ui.tool, Tool::Brush | Tool::MixerBrush) {
        return;
    }
    let Some(name) = app.session.tools.current_preset.clone() else { return };
    if let Some(kit) = presets::find(&app.session.tools.presets, &name).and_then(|p| Kit::of_group(&p.group)) {
        app.ui.kit = kit;
    }
    let tool = tool_for(Some(&name));
    if tool != app.ui.tool {
        let brush = app.session.tools.brush.clone();
        use_brush(app, brush, Some(name));
    }
}

/// The presets the touch picker lists for the drawing tool in use: its own family, then the
/// user's presets (which belong to no family), so a brush of one's own is never out of reach.
pub fn picker_presets(app: &PhotocraftApp) -> Option<Vec<photocraft_engine::paint::BrushPreset>> {
    let kit = active(app)?;
    let own = |p: &&photocraft_engine::paint::BrushPreset| p.group == kit.group() || !p.builtin;
    Some(app.session.tools.presets.iter().filter(own).cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> PhotocraftApp {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.ui.tool = Tool::Brush;
        app
    }

    #[test]
    fn each_tool_starts_on_its_first_brush_and_remembers_the_last() {
        let mut app = app();
        switch(&mut app, Kit::Pen);
        assert_eq!(app.session.tools.current_preset.as_deref(), Some("G-Pen"));
        assert_eq!(app.ui.tool, Tool::Brush);
        app.session.tools.brush.size = 33.0;
        switch(&mut app, Kit::Pencil);
        assert_eq!(app.session.tools.current_preset.as_deref(), Some(presets::FIRST_BRUSH));
        assert!(app.session.tools.brush.texture.enabled, "the pencil has paper grain");
        switch(&mut app, Kit::Pen);
        assert_eq!(app.session.tools.current_preset.as_deref(), Some("G-Pen"));
        assert_eq!(app.session.tools.brush.size, 33.0, "the pen comes back as it was left");
    }

    #[test]
    fn the_brush_mixes_on_the_mixer_brush_and_colours_stay() {
        let mut app = app();
        app.session.tools.brush.color = [0.2, 0.4, 0.6, 1.0];
        switch(&mut app, Kit::Brush);
        assert_eq!(app.session.tools.current_preset.as_deref(), Some("Watercolour"));
        assert_eq!(app.ui.tool, Tool::MixerBrush, "watercolour mixes with the canvas");
        assert!(app.session.tools.brush.mixer.wet > 0.5);
        assert_eq!(app.session.tools.brush.color, [0.2, 0.4, 0.6, 1.0]);
        // Picking gouache in the picker moves back to the Brush Tool.
        app.run("tools.setBrush", serde_json::json!({ "preset": "Dry Gouache" })).unwrap();
        follow_preset(&mut app);
        assert_eq!(app.ui.tool, Tool::Brush);
        assert_eq!(app.session.tools.current_preset.as_deref(), Some("Dry Gouache"));
        assert_eq!(active(&app), Some(Kit::Brush));
    }

    #[test]
    fn a_pen_picked_from_the_panel_makes_the_pen_the_tool() {
        let mut app = app();
        switch(&mut app, Kit::Pencil);
        app.run("tools.setBrush", serde_json::json!({ "preset": "Mapping Pen" })).unwrap();
        follow_preset(&mut app);
        assert_eq!(active(&app), Some(Kit::Pen));
        let listed = picker_presets(&app).unwrap();
        assert!(listed.iter().all(|p| p.group == presets::PENS || !p.builtin));
        assert!(listed.iter().any(|p| p.name == "G-Pen"));
    }
}
