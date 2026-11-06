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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PiraDoc {
    /// A comic page: where the printed page is trimmed and where the safe area is. `None` for
    /// an ordinary document.
    pub page: Option<ComicPage>,
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
