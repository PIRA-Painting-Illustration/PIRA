//! PIRA's 3D layers: posable wooden mannequins (male and female), a library of ready-made poses,
//! and a CPU renderer that draws a [`Scene3d`] into a layer's pixels at any depth and colour mode.
//!
//! * [`render`]: rasterize a scene into a [`photocraft_raster::Surface`].
//! * [`pose`]: picking parts under the pen and posing them (swing, twist, move, mirror, floor).
//! * [`library`]: the built-in poses and the motion-capture pose library.
//! * [`camera`]: the orbit camera and its projection onto the canvas.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod camera;
pub mod library;
pub mod light;
pub mod mannequin;
pub mod pose;
mod presets;
pub mod render;
pub mod retarget;
pub mod skeleton;

use glam::{Mat4, Quat, Vec3};
use mannequin::Mesh;
use photocraft_doc::{Body3d, Camera3d, Figure3d, Scene3d};
use skeleton::{Pose, Skeleton};
use std::sync::OnceLock;

pub use glam;
pub use render::{Quality, Rendered, render_surface};

/// `v`, or `fallback` when it is NaN or infinite.
pub(crate) fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() { v } else { fallback }
}

/// The skeleton and mesh of a body (built once).
pub fn model(body: Body3d) -> &'static (Skeleton, Mesh) {
    static MALE: OnceLock<(Skeleton, Mesh)> = OnceLock::new();
    static FEMALE: OnceLock<(Skeleton, Mesh)> = OnceLock::new();
    let cell = match body {
        Body3d::Male => &MALE,
        Body3d::Female => &FEMALE,
    };
    cell.get_or_init(|| {
        let s = Skeleton::new(body);
        let m = mannequin::build(&s);
        (s, m)
    })
}

/// Where a figure stands: position, turn and size.
pub fn placement(fig: &Figure3d) -> Mat4 {
    let p = Vec3::from_array(fig.position.map(|v| finite_or(v, 0.0).clamp(-1000.0, 1000.0)));
    let scale = finite_or(fig.scale, 1.0).clamp(0.05, 20.0);
    Mat4::from_scale_rotation_translation(Vec3::splat(scale), Quat::from_rotation_y(finite_or(fig.yaw, 0.0)), p)
}

/// Each joint's frame in the scene.
pub fn figure_joints(skel: &Skeleton, fig: &Figure3d) -> Vec<Mat4> {
    let place = placement(fig);
    Pose::of(skel, fig).world(skel).into_iter().map(|m| place * m).collect()
}

/// A new figure in `pose` (a pose library id or built-in name; rest pose when empty or unknown).
pub fn new_figure(body: Body3d, pose: Option<&str>) -> Figure3d {
    let mut fig = Figure3d { body, ..Figure3d::default() };
    let name = pose.filter(|p| !p.is_empty()).unwrap_or("Standing");
    if library::apply(&mut fig, name, pose::Sides::Both).is_err() {
        let _ = library::apply(&mut fig, "Standing", pose::Sides::Both);
    }
    fig
}

/// The lowest and highest points of the visible figures (joints, padded for the body's bulk).
pub fn scene_bounds(scene: &Scene3d) -> Option<(Vec3, Vec3)> {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for fig in scene.figures.iter().filter(|f| f.visible) {
        let (skel, _) = model(fig.body);
        let joints = figure_joints(skel, fig);
        let pad = 0.12 * finite_or(fig.scale, 1.0).clamp(0.05, 20.0);
        for (m, j) in joints.iter().zip(&skel.joints) {
            for p in [m.w_axis.truncate(), m.transform_point3(j.tail)] {
                lo = lo.min(p - pad);
                hi = hi.max(p + pad);
            }
        }
    }
    (lo.is_finite() && hi.is_finite() && lo.x <= hi.x).then_some((lo, hi))
}

/// Points the camera at the figures and backs it off until they fill about `fill` of the canvas
/// height (keeping its angle).
pub fn frame(scene: &mut Scene3d, width: u32, height: u32, fill: f32) {
    let Some((lo, hi)) = scene_bounds(scene) else { return };
    let cam = &mut scene.camera;
    let fill = finite_or(fill, 0.85).clamp(0.1, 1.0);
    cam.target = ((lo + hi) * 0.5).to_array();
    cam.offset_px = [0.0; 2];
    let aspect = width.max(1) as f32 / height.max(1) as f32;
    let size = hi - lo;
    let extent = size.y.max(size.x.max(size.z) / aspect.max(0.05));
    let fov = finite_or(cam.fov_y_deg, 35.0).clamp(5.0, 120.0).to_radians();
    cam.distance = (extent * 0.5 / fill / (fov * 0.5).tan() + size.z.max(size.x) * 0.5).clamp(0.3, 200.0);
}

/// A camera looking from one of the standard directions.
pub fn camera_preset(cam: &Camera3d, view: &str) -> Option<Camera3d> {
    use std::f32::consts::{FRAC_PI_2, PI};
    let (yaw, pitch) = match view {
        "front" => (0.0, 0.0),
        "back" => (PI, 0.0),
        "left" => (FRAC_PI_2, 0.0),
        "right" => (-FRAC_PI_2, 0.0),
        "top" => (0.0, 1.45),
        "bottom" => (0.0, -1.2),
        "threeQuarter" => (0.6, 0.18),
        "high" => (0.5, 0.75),
        "low" => (0.4, -0.35),
        _ => return None,
    };
    Some(Camera3d { yaw, pitch, ..cam.clone() })
}

/// The camera views [`camera_preset`] knows, with labels.
pub const CAMERA_VIEWS: [(&str, &str); 9] = [
    ("front", "Front"),
    ("threeQuarter", "Three-quarter"),
    ("left", "Left side"),
    ("right", "Right side"),
    ("back", "Back"),
    ("top", "Top"),
    ("high", "High angle"),
    ("low", "Low angle"),
    ("bottom", "Worm's eye"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_puts_the_figure_mid_canvas() {
        let mut s = Scene3d { figures: vec![new_figure(Body3d::Female, Some("Walking"))], ..Scene3d::default() };
        frame(&mut s, 1000, 1500, 0.8);
        let r = render::render(&s, 1000, 1500, Quality::Draft);
        let c = r.rect;
        assert!(c.height() > 900 && c.height() < 1450, "{c:?}");
        assert!((c.y0 + c.y1 - 1500).abs() < 200, "{c:?}");
    }

    #[test]
    fn every_camera_view_is_known() {
        let cam = Camera3d::default();
        for (id, _) in CAMERA_VIEWS {
            assert!(camera_preset(&cam, id).is_some(), "{id}");
        }
    }
}
