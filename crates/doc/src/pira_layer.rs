//! PIRA's own layer settings for line-art and comic workflows (not in Photoshop's model). Saved
//! in `.pcraft`; PSD export leaves them out.

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
