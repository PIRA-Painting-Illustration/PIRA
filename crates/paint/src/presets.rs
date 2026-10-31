//! Built-in brush presets (our own designs; sampled tips and textures are generated procedurally).
//!
//! PIRA Essentials comes first (pencils, liners, ink, marker, airbrush, gouache tuned for pen
//! pressure and tilt), then groups like Photoshop's default Brushes panel folders: General, Dry
//! Media, Wet Media and Special Effects, made pen-ready ([`pen_ready`]). Every preset is plain [`BrushSettings`] data, so it serialises, journals and
//! can be edited like any user preset.

use crate::brush::{
    BrushPreset, BrushSettings, ColorDynamics, Control, DualBrush, Dynamic, MaskMode, Pattern, PatternStyle, Scattering, ShapeDynamics, Smoothing, Texture,
    TiltShading, TipShape, Transfer,
};
use crate::procedural::{bristle_tip, chalk_tip, charcoal_tip, grass_tip, leaf_tip, rake_tip, spatter_tip, sponge_tip, star_tip};

/// Preset groups, in panel order.
pub const GROUPS: [&str; 5] = ["PIRA Essentials", "General", "Dry Media", "Wet Media", "Special Effects"];

fn preset(group: &str, name: &str, brush: BrushSettings) -> BrushPreset {
    BrushPreset { name: name.to_string(), brush, builtin: true, group: group.to_string(), folder: Vec::new(), captures: Default::default() }
}

fn pressure() -> Dynamic {
    Dynamic::controlled(Control::PenPressure)
}

fn pressure_min(minimum: f32) -> Dynamic {
    Dynamic { minimum, ..pressure() }
}

fn size_dyn(size: Dynamic) -> ShapeDynamics {
    ShapeDynamics { enabled: true, size, ..Default::default() }
}

fn transfer(opacity: Dynamic, flow: Dynamic) -> Transfer {
    Transfer { enabled: true, opacity, flow, ..Default::default() }
}

fn paper(seed: u32, depth: f32) -> Texture {
    Texture { enabled: true, pattern: Pattern::Procedural { style: PatternStyle::Paper, size: 128, seed }, depth, ..Default::default() }
}

/// The preset a fresh install starts with (nothing remembered from a previous session).
pub const FIRST_BRUSH: &str = "2B Pencil";

fn tilt(size: f32, flatten: f32, opacity: f32) -> TiltShading {
    TiltShading { size, flatten, opacity, ..TiltShading::pencil() }
}

fn smooth(amount: f32) -> Smoothing {
    Smoothing { amount, ..Default::default() }
}

/// PIRA's own starter set: every preset follows pen pressure, and the dry ones shade with tilt,
/// with no setting to switch on first (PIRA tiers 1: graphite, liner, ink, marker, airbrush,
/// gouache).
fn pira_essentials(base: &BrushSettings) -> Vec<BrushPreset> {
    let g = |n: &str, b: BrushSettings| preset("PIRA Essentials", n, b);
    let pencil = |size: f32, hardness: f32, min_size: f32, min_opacity: f32, depth: f32, seed: u32| BrushSettings {
        size,
        hardness,
        spacing: 0.06,
        shape_dynamics: size_dyn(pressure_min(min_size)),
        transfer: transfer(pressure_min(min_opacity), Dynamic::default()),
        texture: Texture { scale: 0.5, ..paper(seed, depth) },
        tilt_shading: TiltShading::pencil(),
        smoothing: smooth(0.15),
        ..base.clone()
    };
    vec![
        g("HB Pencil", BrushSettings { tilt_shading: tilt(1.2, 0.5, 0.45), ..pencil(4.0, 0.75, 0.45, 0.25, 0.6, 21) }),
        g("2B Pencil", BrushSettings { tilt_shading: tilt(1.8, 0.55, 0.4), ..pencil(7.0, 0.6, 0.35, 0.15, 0.75, 22) }),
        g("6B Pencil", BrushSettings { tilt_shading: tilt(2.2, 0.6, 0.35), ..pencil(12.0, 0.45, 0.3, 0.1, 0.85, 23) }),
        g(
            "Mechanical Pencil",
            BrushSettings { spacing: 0.05, tilt_shading: tilt(0.5, 0.3, 0.3), smoothing: smooth(0.25), ..pencil(2.5, 0.9, 0.7, 0.3, 0.4, 29) },
        ),
        g("Colored Pencil", BrushSettings { tilt_shading: tilt(1.6, 0.55, 0.35), ..pencil(8.0, 0.55, 0.5, 0.2, 0.9, 31) }),
        g(
            "Fine Liner",
            BrushSettings { size: 3.0, hardness: 1.0, spacing: 0.04, shape_dynamics: size_dyn(pressure_min(0.8)), smoothing: smooth(0.3), ..base.clone() },
        ),
        g(
            "Inking Pen",
            BrushSettings { size: 10.0, hardness: 1.0, spacing: 0.03, shape_dynamics: size_dyn(pressure_min(0.05)), smoothing: smooth(0.45), ..base.clone() },
        ),
        g(
            "Brush Pen",
            BrushSettings {
                size: 18.0,
                hardness: 0.95,
                spacing: 0.03,
                shape_dynamics: size_dyn(pressure_min(0.02)),
                tilt_shading: TiltShading { start: 30.0, ..tilt(0.8, 0.5, 0.1) },
                smoothing: smooth(0.4),
                ..base.clone()
            },
        ),
        g(
            "Marker",
            BrushSettings {
                size: 24.0,
                hardness: 0.85,
                flow: 0.6,
                spacing: 0.05,
                mode: photocraft_color::BlendMode::Multiply,
                shape_dynamics: size_dyn(pressure_min(0.6)),
                tilt_shading: tilt(0.8, 0.4, 0.15),
                smoothing: smooth(0.2),
                ..base.clone()
            },
        ),
        g(
            "Airbrush",
            BrushSettings {
                size: 120.0,
                hardness: 0.0,
                flow: 0.08,
                spacing: 0.1,
                build_up: true,
                build_up_rate: 25.0,
                transfer: transfer(Dynamic::default(), pressure()),
                tilt_shading: tilt(1.2, 0.5, 0.2),
                ..base.clone()
            },
        ),
        g(
            "Dry Gouache",
            BrushSettings {
                size: 36.0,
                hardness: 0.85,
                spacing: 0.05,
                shape_dynamics: size_dyn(pressure_min(0.5)),
                transfer: transfer(pressure_min(0.5), Dynamic::default()),
                texture: Texture {
                    enabled: true,
                    pattern: Pattern::Procedural { style: PatternStyle::Canvas, size: 64, seed: 6 },
                    depth: 0.45,
                    mode: MaskMode::Subtract,
                    invert: true,
                    ..Default::default()
                },
                tilt_shading: tilt(0.8, 0.4, 0.25),
                ..base.clone()
            },
        ),
    ]
}

/// Make a built-in preset pen-ready: size follows pressure when nothing else drives it (keeping
/// any size jitter), and dry media shades with tilt. Run over the Photoshop-style groups, whose
/// base leaves pressure off, so they all respond to a pen out of the box.
fn pen_ready(mut p: BrushPreset) -> BrushPreset {
    let b = &mut p.brush;
    let sd = &mut b.shape_dynamics;
    let size_driven = (sd.enabled && sd.size.control != Control::Off) || b.pressure_size;
    if !size_driven {
        if !sd.enabled {
            // Only the size dynamic: angle and roundness stay neutral.
            *sd = ShapeDynamics { enabled: true, ..Default::default() };
        }
        sd.size.control = Control::PenPressure;
        sd.size.minimum = sd.size.minimum.max(0.35);
    }
    if p.group == "Dry Media" && b.tilt_shading.is_off() {
        b.tilt_shading = TiltShading::pencil();
    }
    p
}

fn general(base: &BrushSettings) -> Vec<BrushPreset> {
    let g = |n: &str, b: BrushSettings| preset("General", n, b);
    // Dense dabs keep large hard-round outlines and pressure-opacity tapers continuous (#8842).
    let hard_round = BrushSettings { spacing: 0.05, ..base.clone() };
    vec![
        g("Hard Round", BrushSettings { size: 30.0, hardness: 1.0, ..hard_round.clone() }),
        g("Soft Round", BrushSettings { size: 45.0, hardness: 0.0, ..base.clone() }),
        g("Hard Round Fine", BrushSettings { size: 5.0, hardness: 1.0, ..hard_round.clone() }),
        g("Soft Round Large", BrushSettings { size: 300.0, hardness: 0.0, spacing: 0.15, ..base.clone() }),
        g("Hard Round Pressure Size", BrushSettings { size: 30.0, hardness: 1.0, shape_dynamics: size_dyn(pressure()), ..hard_round.clone() }),
        g("Soft Round Pressure Size", BrushSettings { size: 45.0, hardness: 0.0, shape_dynamics: size_dyn(pressure()), ..base.clone() }),
        g("Hard Round Pressure Opacity", BrushSettings { size: 30.0, hardness: 1.0, transfer: transfer(pressure(), Dynamic::default()), ..hard_round.clone() }),
        g("Soft Round Pressure Opacity", BrushSettings { size: 45.0, hardness: 0.0, transfer: transfer(pressure(), Dynamic::default()), ..base.clone() }),
        g("Hard Round Pressure Opacity and Flow", BrushSettings { size: 30.0, hardness: 0.9, transfer: transfer(pressure(), pressure_min(0.2)), ..hard_round }),
        g("Airbrush Soft", BrushSettings { size: 80.0, hardness: 0.0, flow: 0.1, spacing: 0.1, build_up: true, build_up_rate: 25.0, ..base.clone() }),
        g(
            "Airbrush Pressure Flow",
            BrushSettings {
                size: 120.0,
                hardness: 0.0,
                flow: 0.15,
                spacing: 0.1,
                build_up: true,
                build_up_rate: 30.0,
                transfer: transfer(Dynamic::default(), pressure()),
                ..base.clone()
            },
        ),
        g("Calligraphy Flat", BrushSettings { size: 28.0, hardness: 1.0, roundness: 0.2, angle: 45.0, spacing: 0.05, ..base.clone() }),
    ]
}

fn dry_media(base: &BrushSettings) -> Vec<BrushPreset> {
    let g = |n: &str, b: BrushSettings| preset("Dry Media", n, b);
    vec![
        g("Hard Pencil", BrushSettings { size: 3.0, hardness: 1.0, aliased: true, spacing: 0.1, ..base.clone() }),
        g(
            "Soft Pencil",
            BrushSettings {
                size: 6.0,
                hardness: 0.6,
                spacing: 0.08,
                shape_dynamics: size_dyn(pressure_min(0.4)),
                transfer: transfer(pressure_min(0.15), Dynamic::default()),
                texture: Texture { scale: 0.5, ..paper(21, 0.8) },
                ..base.clone()
            },
        ),
        g(
            "Chalk",
            BrushSettings {
                size: 40.0,
                tip: TipShape::Sampled(chalk_tip(64, 7)),
                spacing: 0.2,
                shape_dynamics: ShapeDynamics {
                    enabled: true,
                    angle: Dynamic::jitter(1.0),
                    size: Dynamic { jitter: 0.15, ..Default::default() },
                    ..Default::default()
                },
                texture: paper(3, 0.6),
                ..base.clone()
            },
        ),
        g(
            "Charcoal",
            BrushSettings {
                size: 45.0,
                tip: TipShape::Sampled(charcoal_tip(64, 4)),
                spacing: 0.12,
                shape_dynamics: ShapeDynamics { enabled: true, size: pressure_min(0.5), angle: Dynamic::controlled(Control::Direction), ..Default::default() },
                transfer: transfer(pressure_min(0.3), Dynamic::default()),
                // Subtract takes paint away where the pattern is light (#3340); inverting keeps
                // these presets as they were designed, with the dark parts taking it away.
                texture: Texture { mode: MaskMode::Subtract, invert: true, ..paper(5, 0.7) },
                ..base.clone()
            },
        ),
        g(
            "Pastel",
            BrushSettings {
                size: 50.0,
                tip: TipShape::Sampled(chalk_tip(64, 31)),
                spacing: 0.15,
                shape_dynamics: ShapeDynamics {
                    enabled: true,
                    angle: Dynamic::jitter(1.0),
                    roundness: Dynamic { jitter: 0.3, minimum: 0.5, ..Default::default() },
                    ..Default::default()
                },
                scattering: Scattering { enabled: true, scatter: Dynamic::jitter(0.25), ..Default::default() },
                texture: Texture {
                    enabled: true,
                    pattern: Pattern::Procedural { style: PatternStyle::Canvas, size: 64, seed: 2 },
                    depth: 0.55,
                    mode: MaskMode::Subtract,
                    invert: true,
                    ..Default::default()
                },
                ..base.clone()
            },
        ),
        g(
            "Wax Crayon",
            BrushSettings {
                size: 22.0,
                hardness: 0.85,
                roundness: 0.7,
                spacing: 0.08,
                shape_dynamics: ShapeDynamics { enabled: true, angle: Dynamic::controlled(Control::Direction), ..Default::default() },
                texture: Texture { each_tip: true, mode: MaskMode::HardMix, depth_jitter: Dynamic::jitter(0.3), ..paper(17, 0.9) },
                ..base.clone()
            },
        ),
        g(
            "Dry Bristle",
            BrushSettings {
                size: 50.0,
                tip: TipShape::Sampled(bristle_tip(64, 5)),
                spacing: 0.04,
                shape_dynamics: ShapeDynamics { enabled: true, angle: Dynamic::controlled(Control::Direction), ..Default::default() },
                transfer: Transfer {
                    enabled: true,
                    flow: Dynamic { control: Control::Fade, fade_steps: 400, minimum: 0.1, ..Default::default() },
                    ..Default::default()
                },
                ..base.clone()
            },
        ),
        g(
            "Rake",
            BrushSettings {
                size: 60.0,
                tip: TipShape::Sampled(rake_tip(64, 3, 7)),
                spacing: 0.03,
                shape_dynamics: ShapeDynamics { enabled: true, angle: Dynamic::controlled(Control::Direction), ..Default::default() },
                transfer: transfer(pressure_min(0.25), Dynamic::default()),
                ..base.clone()
            },
        ),
    ]
}

fn wet_media(base: &BrushSettings) -> Vec<BrushPreset> {
    let g = |n: &str, b: BrushSettings| preset("Wet Media", n, b);
    vec![
        g(
            "Watercolor Wet Edges",
            BrushSettings {
                size: 60.0,
                hardness: 0.3,
                wet_edges: true,
                texture: Texture { scale: 1.5, ..paper(9, 0.35) }.with_pattern_size(256),
                ..base.clone()
            },
        ),
        g(
            "Watercolor Wash",
            BrushSettings {
                size: 140.0,
                hardness: 0.0,
                flow: 0.35,
                spacing: 0.12,
                wet_edges: true,
                shape_dynamics: size_dyn(pressure_min(0.35)),
                transfer: transfer(Dynamic::default(), Dynamic { jitter: 0.2, ..pressure() }),
                texture: Texture { scale: 2.0, ..paper(13, 0.25) }.with_pattern_size(256),
                ..base.clone()
            },
        ),
        g(
            "Ink Pen",
            BrushSettings {
                size: 8.0,
                hardness: 1.0,
                spacing: 0.05,
                shape_dynamics: size_dyn(pressure_min(0.15)),
                smoothing: Smoothing { amount: 0.35, ..Default::default() },
                ..base.clone()
            },
        ),
        g(
            "Oil Bristle",
            BrushSettings {
                size: 70.0,
                tip: TipShape::Sampled(bristle_tip(64, 41)),
                spacing: 0.03,
                shape_dynamics: ShapeDynamics { enabled: true, angle: Dynamic::controlled(Control::Direction), size: pressure_min(0.6), ..Default::default() },
                color_dynamics: ColorDynamics { enabled: true, per_tip: false, fg_bg: Dynamic::jitter(0.15), brightness_jitter: 0.05, ..Default::default() },
                texture: Texture {
                    enabled: true,
                    pattern: Pattern::Procedural { style: PatternStyle::Canvas, size: 64, seed: 4 },
                    depth: 0.3,
                    ..Default::default()
                },
                ..base.clone()
            },
        ),
        g(
            "Gouache Flat",
            BrushSettings {
                size: 40.0,
                hardness: 0.9,
                roundness: 0.35,
                spacing: 0.05,
                shape_dynamics: ShapeDynamics { enabled: true, angle: Dynamic::controlled(Control::Direction), ..Default::default() },
                transfer: transfer(Dynamic::default(), Dynamic { control: Control::Fade, fade_steps: 600, minimum: 0.35, ..Default::default() }),
                ..base.clone()
            },
        ),
        g(
            "Ink Wash Sponge",
            BrushSettings {
                size: 70.0,
                tip: TipShape::Sampled(sponge_tip(96, 8)),
                spacing: 0.3,
                flow: 0.6,
                wet_edges: true,
                shape_dynamics: ShapeDynamics { enabled: true, angle: Dynamic::jitter(1.0), size: Dynamic::jitter(0.2), ..Default::default() },
                ..base.clone()
            },
        ),
    ]
}

fn special_effects(base: &BrushSettings) -> Vec<BrushPreset> {
    let g = |n: &str, b: BrushSettings| preset("Special Effects", n, b);
    vec![
        g(
            "Spatter",
            BrushSettings {
                size: 60.0,
                tip: TipShape::Sampled(spatter_tip(64, 11, 14)),
                spacing: 0.6,
                shape_dynamics: ShapeDynamics {
                    enabled: true,
                    size: Dynamic { jitter: 0.6, minimum: 0.2, ..Default::default() },
                    angle: Dynamic::jitter(1.0),
                    flip_x_jitter: true,
                    flip_y_jitter: true,
                    ..Default::default()
                },
                scattering: Scattering { enabled: true, scatter: Dynamic::jitter(1.5), both_axes: true, count: 2, count_jitter: Dynamic::jitter(0.5) },
                ..base.clone()
            },
        ),
        g(
            "Stipple",
            BrushSettings {
                size: 40.0,
                tip: TipShape::Sampled(spatter_tip(64, 77, 40)),
                spacing: 0.5,
                shape_dynamics: ShapeDynamics { enabled: true, angle: Dynamic::jitter(1.0), flip_x_jitter: true, ..Default::default() },
                scattering: Scattering { enabled: true, scatter: Dynamic::jitter(0.6), both_axes: true, ..Default::default() },
                transfer: transfer(pressure(), Dynamic::default()),
                ..base.clone()
            },
        ),
        g(
            "Confetti",
            BrushSettings {
                size: 24.0,
                hardness: 1.0,
                roundness: 0.5,
                spacing: 1.2,
                shape_dynamics: ShapeDynamics { enabled: true, angle: Dynamic::jitter(1.0), size: Dynamic::jitter(0.5), ..Default::default() },
                scattering: Scattering { enabled: true, scatter: Dynamic::jitter(3.0), both_axes: true, count: 3, ..Default::default() },
                color_dynamics: ColorDynamics { enabled: true, per_tip: true, hue_jitter: 1.0, saturation_jitter: 0.2, purity: 0.5, ..Default::default() },
                ..base.clone()
            },
        ),
        g(
            "Foliage",
            BrushSettings {
                size: 50.0,
                tip: TipShape::Sampled(leaf_tip(64)),
                spacing: 0.35,
                shape_dynamics: ShapeDynamics {
                    enabled: true,
                    size: Dynamic { jitter: 0.5, minimum: 0.3, ..Default::default() },
                    angle: Dynamic::jitter(1.0),
                    roundness: Dynamic { jitter: 0.4, minimum: 0.4, ..Default::default() },
                    flip_y_jitter: true,
                    ..Default::default()
                },
                scattering: Scattering { enabled: true, scatter: Dynamic::jitter(1.2), both_axes: true, count: 2, count_jitter: Dynamic::jitter(0.5) },
                color_dynamics: ColorDynamics {
                    enabled: true,
                    per_tip: true,
                    fg_bg: Dynamic::jitter(0.6),
                    hue_jitter: 0.04,
                    brightness_jitter: 0.12,
                    ..Default::default()
                },
                ..base.clone()
            },
        ),
        g(
            "Grass",
            BrushSettings {
                size: 70.0,
                tip: TipShape::Sampled(grass_tip(96, 6, 11)),
                spacing: 0.25,
                shape_dynamics: ShapeDynamics {
                    enabled: true,
                    size: Dynamic { jitter: 0.4, minimum: 0.4, ..pressure() },
                    angle: Dynamic::jitter(0.06),
                    flip_x_jitter: true,
                    ..Default::default()
                },
                scattering: Scattering { enabled: true, scatter: Dynamic::jitter(0.3), ..Default::default() },
                color_dynamics: ColorDynamics { enabled: true, per_tip: true, fg_bg: Dynamic::jitter(0.7), hue_jitter: 0.03, ..Default::default() },
                ..base.clone()
            },
        ),
        g(
            "Sparkle",
            BrushSettings {
                size: 40.0,
                tip: TipShape::Sampled(star_tip(64)),
                spacing: 1.5,
                shape_dynamics: ShapeDynamics { enabled: true, size: Dynamic { jitter: 0.8, minimum: 0.15, ..Default::default() }, ..Default::default() },
                scattering: Scattering { enabled: true, scatter: Dynamic::jitter(2.5), both_axes: true, ..Default::default() },
                transfer: transfer(Dynamic::jitter(0.5), Dynamic::default()),
                ..base.clone()
            },
        ),
        g(
            "Sponge Texture",
            BrushSettings {
                size: 60.0,
                tip: TipShape::Sampled(sponge_tip(64, 19)),
                spacing: 0.45,
                shape_dynamics: ShapeDynamics { enabled: true, angle: Dynamic::jitter(1.0), ..Default::default() },
                transfer: transfer(Dynamic::jitter(0.3), Dynamic::default()),
                ..base.clone()
            },
        ),
        g(
            "Canvas Texture",
            BrushSettings {
                size: 50.0,
                hardness: 0.8,
                texture: Texture {
                    enabled: true,
                    pattern: Pattern::Procedural { style: PatternStyle::Canvas, size: 64, seed: 1 },
                    mode: MaskMode::Subtract,
                    invert: true,
                    depth: 0.7,
                    ..Default::default()
                },
                ..base.clone()
            },
        ),
        g(
            "Halftone Dots",
            BrushSettings {
                size: 80.0,
                hardness: 0.5,
                texture: Texture {
                    enabled: true,
                    pattern: Pattern::Procedural { style: PatternStyle::Dots, size: 64, seed: 1 },
                    mode: MaskMode::HardMix,
                    depth: 1.0,
                    ..Default::default()
                },
                ..base.clone()
            },
        ),
        g(
            "Dual Grain",
            BrushSettings {
                size: 50.0,
                hardness: 0.6,
                noise: true,
                dual_brush: DualBrush {
                    enabled: true,
                    tip: TipShape::Sampled(spatter_tip(48, 23, 20)),
                    size: 30.0,
                    spacing: 0.3,
                    scatter: 1.0,
                    both_axes: true,
                    count: 2,
                    flip: true,
                    ..Default::default()
                },
                ..base.clone()
            },
        ),
    ]
}

impl Texture {
    fn with_pattern_size(mut self, n: u32) -> Self {
        if let Pattern::Procedural { size, .. } = &mut self.pattern {
            *size = n;
        }
        self
    }
}

/// The built-in preset set, in display order (grouped by [`GROUPS`]).
pub fn builtin() -> Vec<BrushPreset> {
    // The Android app is drawn on with a pen: its brushes follow the pen out of the box.
    if cfg!(target_os = "android") { for_pen(desktop()) } else { desktop() }
}

/// The built-in brushes as the desktop has them (Photoshop's set).
fn desktop() -> Vec<BrushPreset> {
    let base = BrushSettings { pressure_size: false, spacing: 0.25, ..Default::default() };
    let mut v = pira_essentials(&base);
    v.extend(general(&base));
    v.extend(dry_media(&base).into_iter().map(pen_ready));
    v.extend(wet_media(&base).into_iter().map(pen_ready));
    v.extend(special_effects(&base));
    v
}

/// Brushes that stamp or scatter fixed shapes: their size stays put under pressure.
const FIXED_SIZE: [&str; 9] = ["Spatter", "Stipple", "Confetti", "Sparkle", "Foliage", "Halftone Dots", "Sponge Texture", "Canvas Texture", "Dual Grain"];

/// Drawing tools whose tip leans with the pen (Brush Projection), like a real pencil, crayon or
/// nib: tilt it to shade with the side.
const LEANING: [&str; 10] =
    ["Calligraphy Flat", "Hard Pencil", "Soft Pencil", "Chalk", "Charcoal", "Pastel", "Wax Crayon", "Dry Bristle", "Rake", "Oil Bristle"];

/// The built-in brushes set up for a pen (the Android app's): pressure sets the size of every
/// brush but those that stamp fixed shapes, and the drawing tools' tips lean with the pen. A
/// brush whose size already follows the pen keeps its own minimum.
pub fn for_pen(mut presets: Vec<BrushPreset>) -> Vec<BrushPreset> {
    for p in &mut presets {
        let sd = &mut p.brush.shape_dynamics;
        if !FIXED_SIZE.contains(&p.name.as_str()) && !p.brush.pressure_size && !(sd.enabled && sd.size.control == Control::PenPressure) {
            if !sd.enabled {
                // Shape Dynamics off: only its size is turned on, nothing else it holds.
                *sd = ShapeDynamics::default();
                sd.enabled = true;
            }
            sd.size.control = Control::PenPressure;
        }
        if LEANING.contains(&p.name.as_str()) {
            sd.enabled = true;
            sd.brush_projection = true;
        }
    }
    presets
}

/// Find a preset by name (case-insensitive).
pub fn find<'a>(presets: &'a [BrushPreset], name: &str) -> Option<&'a BrushPreset> {
    presets.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set up for a pen: size follows pressure on all but the stamp-like brushes, the drawing
    /// tools lean with the pen, and nothing else about any brush changes.
    #[test]
    fn brushes_for_a_pen_follow_pressure_and_tilt() {
        let plain = desktop();
        let pen = for_pen(plain.clone());
        assert_eq!(pen.len(), plain.len());
        let sizes = |b: &BrushSettings| b.pressure_size || (b.shape_dynamics.enabled && b.shape_dynamics.size.control == Control::PenPressure);
        for (before, after) in plain.iter().zip(&pen) {
            let name = before.name.as_str();
            if FIXED_SIZE.contains(&name) {
                assert_eq!(sizes(&after.brush), sizes(&before.brush), "{name} keeps its size");
            } else {
                assert!(sizes(&after.brush), "{name}: pressure sets the size");
            }
            assert_eq!(after.brush.shape_dynamics.brush_projection, LEANING.contains(&name), "{name}");
            if sizes(&before.brush) {
                // Already followed the pen: its own minimum stays.
                assert_eq!(after.brush.shape_dynamics.size.minimum, before.brush.shape_dynamics.size.minimum, "{name}");
            }
            if before.brush.shape_dynamics.enabled {
                assert_eq!(after.brush.shape_dynamics.angle, before.brush.shape_dynamics.angle, "{name}: its angle dynamics stay");
            }
            assert_eq!(
                (after.brush.size, after.brush.hardness, after.brush.tip.clone()),
                (before.brush.size, before.brush.hardness, before.brush.tip.clone()),
                "{name}"
            );
        }
        for name in FIXED_SIZE.iter().chain(&LEANING) {
            assert!(plain.iter().any(|p| p.name == *name), "{name} is a built-in brush");
        }
    }

    #[test]
    fn builtins_are_grouped_and_unique() {
        let v = builtin();
        assert!((40..=56).contains(&v.len()), "{}", v.len());
        let mut names: Vec<&str> = v.iter().map(|p| p.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), v.len(), "duplicate preset names");
        for g in GROUPS {
            assert!(v.iter().filter(|p| p.group == g).count() >= 6, "{g}");
        }
        assert!(v.iter().all(|p| p.builtin && GROUPS.contains(&p.group.as_str())));
        // Groups are contiguous, in panel order.
        let order: Vec<usize> = v.iter().filter_map(|p| GROUPS.iter().position(|g| *g == p.group)).collect();
        assert!(order.windows(2).all(|w| w[0] <= w[1]));
        // Pressure variants respond to pressure.
        let ps = find(&v, "Hard Round Pressure Size").map(|p| p.brush.shape_dynamics.size.control);
        assert_eq!(ps, Some(Control::PenPressure));
        assert!(find(&v, FIRST_BRUSH).is_some());
    }

    fn at(x: f64, pressure: f32, tilt_x: f32) -> crate::StrokePoint {
        crate::StrokePoint { x, y: 0.0, pressure, tilt_x, ..Default::default() }
    }

    fn step(p: crate::StrokePoint) -> crate::dynamics::StepInput {
        crate::dynamics::StepInput { point: p, direction: 0.0, initial_direction: 0.0, step: 0 }
    }

    fn dab(b: &BrushSettings, p: crate::StrokePoint) -> crate::Dab {
        let mut out = Vec::new();
        crate::dynamics::DabBuilder::new(b).build(&step(p), &mut out);
        out.remove(0)
    }

    /// Every pencil, ink and dry or wet media preset changes size with pressure, with no
    /// setting to turn on (PIRA's out-of-the-box promise).
    #[test]
    fn drawing_presets_follow_pressure_out_of_the_box() {
        for p in builtin().iter().filter(|p| ["PIRA Essentials", "Dry Media", "Wet Media"].contains(&p.group.as_str())) {
            let light = crate::dynamics::base_diameter(&p.brush, &step(at(0.0, 0.1, 0.0)));
            let firm = crate::dynamics::base_diameter(&p.brush, &step(at(0.0, 1.0, 0.0)));
            let flow_driven = p.brush.transfer.enabled && p.brush.transfer.flow.control == Control::PenPressure;
            assert!(firm > light || flow_driven, "{}: {light} vs {firm}", p.name);
        }
    }

    /// The pencils shade with tilt by default: upright stays crisp, leaning grows a longer,
    /// flatter, lighter tip along the lean; a mouse (no tilt) paints as upright.
    #[test]
    fn pencils_shade_with_tilt_by_default() {
        let v = builtin();
        for name in ["HB Pencil", "2B Pencil", "6B Pencil", "Soft Pencil", "Charcoal"] {
            let b = &find(&v, name).unwrap().brush;
            assert!(b.tilt_shading.enabled, "{name}");
            let upright = dab(b, at(0.0, 0.8, 0.0));
            let writing = dab(b, at(0.0, 0.8, 20.0));
            let leaning = dab(b, at(0.0, 0.8, 60.0));
            assert_eq!(upright.radius, writing.radius, "{name}: a writing grip stays crisp");
            assert!(leaning.radius > upright.radius * 1.4, "{name}: {} vs {}", leaning.radius, upright.radius);
            assert!(leaning.opacity < upright.opacity, "{name}");
            if name != "Charcoal" {
                assert!(leaning.roundness < upright.roundness, "{name}");
            }
        }
    }

    #[test]
    fn tilt_shading_follows_the_lean_and_ignores_hostile_numbers() {
        let b = BrushSettings { tilt_shading: TiltShading::pencil(), ..Default::default() };
        let along_x = dab(&b, at(0.0, 1.0, 60.0));
        let along_y = dab(&b, crate::StrokePoint { tilt_y: 60.0, ..at(0.0, 1.0, 0.0) });
        assert!(along_x.angle.abs() < 1e-4);
        assert!((along_y.angle.abs() - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let d = dab(&b, at(0.0, 1.0, bad));
            assert!(d.radius.is_finite() && d.opacity.is_finite() && d.roundness.is_finite());
        }
        let odd = BrushSettings { tilt_shading: TiltShading { start: 80.0, full: 10.0, size: f32::NAN, ..TiltShading::pencil() }, ..Default::default() };
        assert!(dab(&odd, at(0.0, 1.0, 90.0)).radius.is_finite());
    }

    #[test]
    fn tilt_shading_off_is_left_out_of_saved_brushes() {
        let off = serde_json::to_value(BrushSettings::default()).unwrap();
        assert!(off.get("tiltShading").is_none());
        let on = serde_json::to_value(BrushSettings { tilt_shading: TiltShading::pencil(), ..Default::default() }).unwrap();
        let back: BrushSettings = serde_json::from_value(on).unwrap();
        assert!(back.tilt_shading.enabled);
    }
}
