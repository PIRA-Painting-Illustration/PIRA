//! 3D layers: posable figures seen through a camera, rendered into the layer like a text layer's
//! glyphs. Pure data; `photocraft-scene3d` renders it and the engine refreshes [`Scene3dLayer::cache`]
//! whenever the scene changes.
//!
//! World units are metres, +Y is up, and a figure at rest faces +Z with its left side towards +X.

use photocraft_raster::Surface;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The mannequin's build.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Body3d {
    #[default]
    Male,
    Female,
}

impl Body3d {
    pub fn label(self) -> &'static str {
        match self {
            Body3d::Male => "Male",
            Body3d::Female => "Female",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Body3d::Male => "male",
            Body3d::Female => "female",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id.to_ascii_lowercase().as_str() {
            "male" | "man" => Some(Body3d::Male),
            "female" | "woman" => Some(Body3d::Female),
            _ => None,
        }
    }
}

/// One posed figure in a 3D layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Figure3d {
    pub body: Body3d,
    /// Offset of the hips from their rest height, in the figure's own frame (metres).
    pub root: [f32; 3],
    /// Each joint's rotation relative to its parent, as an `[x, y, z, w]` quaternion, by joint name
    /// (`hips`, `upper_arm.L`, `index2.R`, …). Joints left out stay at rest (a T-pose).
    pub joints: BTreeMap<String, [f32; 4]>,
    /// Where the figure stands in the scene (metres).
    pub position: [f32; 3],
    /// Turn about the vertical axis (radians).
    pub yaw: f32,
    /// Overall size (1 = a 1.78 m man or a 1.66 m woman).
    pub scale: f32,
    pub visible: bool,
}

impl Default for Figure3d {
    fn default() -> Self {
        Self { body: Body3d::Male, root: [0.0; 3], joints: BTreeMap::new(), position: [0.0; 3], yaw: 0.0, scale: 1.0, visible: true }
    }
}

/// An orbit camera looking at `target`. The layer's image is the whole canvas: `fov_y_deg` spans
/// the canvas height.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Camera3d {
    pub target: [f32; 3],
    /// Angle around the vertical axis (radians, 0 = in front of the scene, looking down −Z).
    pub yaw: f32,
    /// Angle above the floor (radians).
    pub pitch: f32,
    /// Distance from `target` (metres).
    pub distance: f32,
    /// Vertical field of view (degrees). Ignored by a parallel camera.
    pub fov_y_deg: f32,
    /// Parallel (orthographic) projection instead of perspective.
    pub parallel: bool,
    /// Where the camera's centre lands, in canvas pixels from the canvas centre (the Move tool
    /// slides a 3D layer by changing this).
    pub offset_px: [f32; 2],
}

impl Default for Camera3d {
    fn default() -> Self {
        Self { target: [0.0, 0.92, 0.0], yaw: 0.45, pitch: 0.1, distance: 4.2, fov_y_deg: 35.0, parallel: false, offset_px: [0.0; 2] }
    }
}

/// How the figures are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Shading3d {
    /// Soft lighting with highlights.
    #[default]
    Shaded,
    /// Two or three flat tones, like cel shading.
    Toon,
    /// Plain body colour, no lighting.
    Flat,
    /// One solid colour (for checking a pose's silhouette).
    Silhouette,
}

impl Shading3d {
    pub const ALL: [Shading3d; 4] = [Shading3d::Shaded, Shading3d::Toon, Shading3d::Flat, Shading3d::Silhouette];

    pub fn id(self) -> &'static str {
        match self {
            Shading3d::Shaded => "shaded",
            Shading3d::Toon => "toon",
            Shading3d::Flat => "flat",
            Shading3d::Silhouette => "silhouette",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Shading3d::Shaded => "Shaded",
            Shading3d::Toon => "Toon",
            Shading3d::Flat => "Flat",
            Shading3d::Silhouette => "Silhouette",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }
}

/// Lighting, colours and outline of a 3D layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look3d {
    pub shading: Shading3d,
    /// Body colour (sRGB, 0–1).
    pub body_color: [f32; 3],
    /// Outline width in canvas pixels (0 = none).
    pub outline_px: f32,
    /// Outline colour (sRGB, 0–1).
    pub outline_color: [f32; 3],
    /// Direction the main light comes from: around the vertical (radians, 0 = from the front) and
    /// above the floor (radians).
    pub light_yaw: f32,
    pub light_pitch: f32,
    /// With `false` (the default) the light turns with the camera, so the figure stays lit the same
    /// way on the canvas as you orbit, and `light_yaw` is measured from the camera; with `true` it
    /// stays put in the scene and `light_yaw` is measured from the scene's front.
    pub light_fixed: bool,
    /// Main light brightness (0–2, 1 = normal).
    pub light_strength: f32,
    /// Main light colour (sRGB, 0–1).
    pub light_color: [f32; 3],
    /// Ambient light (0–1).
    pub ambient: f32,
    /// A soft contact shadow under each figure.
    pub floor_shadow: bool,
}

impl Default for Look3d {
    fn default() -> Self {
        Self {
            shading: Shading3d::Shaded,
            body_color: [0.80, 0.64, 0.47],
            outline_px: 0.0,
            outline_color: [0.08, 0.07, 0.07],
            light_yaw: 0.6,
            light_pitch: 0.9,
            light_fixed: false,
            light_strength: 1.0,
            light_color: [1.0, 1.0, 1.0],
            ambient: 0.35,
            floor_shadow: true,
        }
    }
}

/// Everything a 3D layer shows.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Scene3d {
    pub figures: Vec<Figure3d>,
    pub camera: Camera3d,
    pub look: Look3d,
}

/// A 3D layer: its scene plus the scene rendered at the canvas size, in the document's pixel format.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene3dLayer {
    pub scene: Scene3d,
    /// Rendered appearance (refreshed by the engine after every edit; `None` until first rendered).
    pub cache: Option<Surface>,
}

impl Scene3dLayer {
    pub fn new(scene: Scene3d) -> Self {
        Self { scene, cache: None }
    }

    /// Slides the rendered image by whole pixels; `shift_cache` moves the cached render the same
    /// way so nothing needs re-rendering.
    pub fn shift(&mut self, dx: i32, dy: i32, shift_cache: impl FnOnce(&Surface) -> Surface) {
        let o = &mut self.scene.camera.offset_px;
        o[0] += dx as f32;
        o[1] += dy as f32;
        if let Some(c) = &mut self.cache {
            *c = shift_cache(c);
        }
    }
}
