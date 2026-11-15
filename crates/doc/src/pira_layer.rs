//! PIRA's own layer and document settings for line-art and comic workflows (not in Photoshop's
//! model). Saved in `.pcraft`; PSD export leaves them out.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PiraLayer {
    /// Reference layer: the Paint Bucket and Magic Wand can look only at reference layers (the
    /// line art) to find the area to fill, whichever layer they fill.
    pub reference: bool,
    /// Draft layer: kept in `.pcraft` and shown while you work, but left out of exported and
    /// printed images and of what the Paint Bucket and Magic Wand look at.
    pub draft: bool,
    /// Line colour: the layer's pixels show in this colour (sRGB), alpha kept, as a Color
    /// Overlay effect the command keeps in step. `None` = off.
    pub line_color: Option<[u8; 3]>,
    /// Border effect: an outline around the layer's pixels, as an outside Stroke effect the
    /// command keeps in step. `None` = off.
    pub border: Option<Border>,
    /// A comic panel: a group whose vector mask is the panel's outline (`comic.newPanels`).
    pub panel: bool,
    /// A speech balloon: a group of its outline shape and its words (`comic.newBalloon`).
    pub balloon: bool,
}

/// A layer's border effect: width in pixels and colour (sRGB).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Border {
    pub width: u16,
    pub color: [u8; 3],
}

impl PiraLayer {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// PIRA's own document settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PiraDoc {
    /// A comic page: where the printed page is trimmed and where the safe area is. `None` for
    /// an ordinary document.
    pub page: Option<ComicPage>,
    /// Special rulers (the Ruler tool's Special Ruler option): lines and curves strokes snap to.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rulers: Vec<SpecialRuler>,
}

impl PiraDoc {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// A comic page's print geometry, in pixels: the canvas is the finished (trim) size plus `bleed`
/// on every side, and the safe area is `safe` inside the trim, where text and faces belong.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComicPage {
    pub bleed: u32,
    pub safe: u32,
}

impl ComicPage {
    /// The trim rectangle `[x0, y0, x1, y1]` of a `width` × `height` canvas.
    pub fn trim(&self, width: u32, height: u32) -> [f64; 4] {
        let b = f64::from(self.bleed);
        [b, b, f64::from(width) - b, f64::from(height) - b]
    }

    /// The safe area `[x0, y0, x1, y1]` of a `width` × `height` canvas.
    pub fn safe_area(&self, width: u32, height: u32) -> [f64; 4] {
        let t = self.trim(width, height);
        let m = f64::from(self.safe);
        [t[0] + m, t[1] + m, t[2] - m, t[3] - m]
    }
}

/// A special ruler: a guide painting strokes snap to. It belongs to a
/// layer and shows (and snaps) only while that layer is active, or on every layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecialRuler {
    pub shape: RulerShape,
    /// The layer it belongs to; `None` = every layer.
    #[serde(default)]
    pub layer: Option<crate::LayerId>,
}

/// A special ruler's geometry, in document pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RulerShape {
    /// The line through `a` and `b`: strokes near it run along it.
    Straight { a: [f64; 2], b: [f64; 2] },
    /// A smooth curve through its points: strokes near it follow it.
    Curve { points: Vec<[f64; 2]> },
    /// Ellipses around `center` with the shape of `radii` (turned by `angle` radians): a stroke
    /// runs round the one through where it starts.
    Concentric { center: [f64; 2], radii: [f64; 2], angle: f64 },
    /// Lines parallel to `a`→`b`, through wherever a stroke starts.
    Parallel { a: [f64; 2], b: [f64; 2] },
    /// Lines through `center`: strokes run towards or away from it.
    Radial { center: [f64; 2] },
    /// Fisheye perspective: arcs through the two vanishing points `a` and `b`, one through
    /// wherever a stroke starts.
    Fisheye { a: [f64; 2], b: [f64; 2] },
}

impl RulerShape {
    /// Every point the ruler is defined by.
    pub fn points(&self) -> Vec<[f64; 2]> {
        match self {
            RulerShape::Straight { a, b } | RulerShape::Parallel { a, b } | RulerShape::Fisheye { a, b } => vec![*a, *b],
            RulerShape::Curve { points } => points.clone(),
            RulerShape::Concentric { center, .. } | RulerShape::Radial { center } => vec![*center],
        }
    }

    /// The kind's name, as the commands take it.
    pub fn kind(&self) -> &'static str {
        match self {
            RulerShape::Straight { .. } => "straight",
            RulerShape::Curve { .. } => "curve",
            RulerShape::Concentric { .. } => "concentric",
            RulerShape::Parallel { .. } => "parallel",
            RulerShape::Radial { .. } => "radial",
            RulerShape::Fisheye { .. } => "fisheye",
        }
    }

    /// Whether the ruler is usable on a `w` × `h` canvas: finite numbers near the canvas, two
    /// distinct points where it needs a direction, 2 to 64 curve points and non-zero radii.
    pub fn ok(&self, w: f64, h: f64) -> bool {
        let limit = crate::PERSPECTIVE_RULER_REACH * w.max(h).max(1.0);
        let fine = |p: &[f64; 2]| p.iter().all(|v| v.is_finite() && v.abs() <= limit);
        let apart = |a: &[f64; 2], b: &[f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) > 1e-6;
        self.points().iter().all(fine)
            && match self {
                RulerShape::Straight { a, b } | RulerShape::Parallel { a, b } | RulerShape::Fisheye { a, b } => apart(a, b),
                RulerShape::Curve { points } => (2..=64).contains(&points.len()) && points.windows(2).any(|w| apart(&w[0], &w[1])),
                RulerShape::Concentric { radii, angle, .. } => fine(radii) && radii.iter().all(|r| *r > 0.5) && angle.is_finite(),
                RulerShape::Radial { .. } => true,
            }
    }
}
