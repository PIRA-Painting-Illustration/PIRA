//! The main light: its direction in the scene, and placing it from the canvas.
//!
//! On the canvas the light is shown as a sun on a disc around the figures: the disc's centre is
//! straight in front (the light comes from the camera), its rim is side-on, and twice the radius
//! is straight behind (rim light). [`from_screen`] and [`handle`] map between the two.

use crate::camera::View;
use crate::finite_or;
use glam::{Vec2, Vec3};
use photocraft_doc::Scene3d;
use std::f32::consts::{FRAC_PI_2, PI};

/// Steepest light, above or below (radians; about 85°).
pub const MAX_PITCH: f32 = 1.48;

/// Unit vector from the figures toward the main light, in the scene.
pub fn direction(scene: &Scene3d) -> Vec3 {
    let look = &scene.look;
    let yaw = finite_or(look.light_yaw, 0.6) + if look.light_fixed { 0.0 } else { finite_or(scene.camera.yaw, 0.0) };
    let pitch = finite_or(look.light_pitch, 0.9).clamp(-MAX_PITCH, MAX_PITCH);
    Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos()).normalize_or(Vec3::Y)
}

/// Points the main light along `dir` (toward the light, in the scene).
pub fn set_direction(scene: &mut Scene3d, dir: Vec3) {
    let d = dir.normalize_or(Vec3::Y);
    let pitch = d.y.clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH);
    let mut yaw = if d.x.abs() + d.z.abs() < 1e-5 { finite_or(scene.look.light_yaw, 0.0) } else { d.x.atan2(d.z) };
    if !scene.look.light_fixed && (d.x.abs() + d.z.abs() >= 1e-5) {
        yaw -= finite_or(scene.camera.yaw, 0.0);
    }
    scene.look.light_yaw = wrap(yaw);
    scene.look.light_pitch = pitch;
}

/// Switches between a light that turns with the camera and one fixed in the scene, keeping it
/// where it is now.
pub fn set_fixed(scene: &mut Scene3d, fixed: bool) {
    if scene.look.light_fixed == fixed {
        return;
    }
    let d = direction(scene);
    scene.look.light_fixed = fixed;
    set_direction(scene, d);
}

/// `a` in (-π, π].
fn wrap(a: f32) -> f32 {
    let a = finite_or(a, 0.0);
    let w = (a + PI).rem_euclid(2.0 * PI) - PI;
    if w <= -PI { w + 2.0 * PI } else { w }
}

/// The light disc on the canvas: centre and radius (canvas pixels).
pub fn disc(scene: &Scene3d, width: u32, height: u32) -> (Vec2, f32) {
    let view = View::new(&scene.camera, width, height);
    let fallback = Vec2::new(width as f32 * 0.5, height as f32 * 0.5);
    let centre = crate::scene_bounds(scene).and_then(|(lo, hi)| view.project((lo + hi) * 0.5)).map_or(fallback, |p| p.px);
    let centre = if centre.is_finite() { centre } else { fallback };
    (centre, width.min(height).max(1) as f32 * 0.32)
}

/// The light direction for the pointer at `px` (canvas pixels) on the light disc.
pub fn from_screen(scene: &Scene3d, width: u32, height: u32, px: [f32; 2]) -> Vec3 {
    let (c, r) = disc(scene, width, height);
    let uv = (Vec2::from_array(px) - c) / r.max(1.0);
    let uv = if uv.is_finite() { uv } else { Vec2::ZERO };
    from_disc(&View::new(&scene.camera, width, height), uv)
}

/// The light direction for a point on the unit light disc (x right, y down; 1 = side-on, 2 = behind).
fn from_disc(view: &View, uv: Vec2) -> Vec3 {
    let (right, up, fwd) = view.axes();
    let toward = -fwd;
    let r = uv.length().min(2.0);
    if r < 1e-5 {
        return toward;
    }
    let theta = r * FRAC_PI_2;
    let side = (right * uv.x - up * uv.y) / uv.length();
    (toward * theta.cos() + side * theta.sin()).normalize_or(toward)
}

/// Where the sun sits on the canvas (canvas pixels), and whether the light is behind the figures.
pub fn handle(scene: &Scene3d, width: u32, height: u32) -> ([f32; 2], bool) {
    let (c, r) = disc(scene, width, height);
    let view = View::new(&scene.camera, width, height);
    let (right, up, fwd) = view.axes();
    let d = direction(scene);
    let theta = d.dot(-fwd).clamp(-1.0, 1.0).acos();
    let side = Vec2::new(d.dot(right), -d.dot(up));
    let dir = side.try_normalize().unwrap_or(Vec2::new(0.0, -1.0));
    let p = c + dir * (theta / FRAC_PI_2) * r;
    (p.to_array(), theta > FRAC_PI_2)
}

/// Ready-made light directions: id, label, and the spot on the light disc they stand for.
pub const PRESETS: [(&str, &str, [f32; 2]); 9] = [
    ("upperLeft", "Upper left", [-0.5, -0.55]),
    ("upperRight", "Upper right", [0.5, -0.55]),
    ("front", "Front", [0.0, 0.0]),
    ("left", "Left side", [-1.0, 0.0]),
    ("right", "Right side", [1.0, 0.0]),
    ("top", "Overhead", [0.0, -0.97]),
    ("below", "From below", [0.0, 0.75]),
    ("rimLeft", "Rim, behind left", [-1.15, -1.0]),
    ("rimRight", "Rim, behind right", [1.15, -1.0]),
];

/// Points the light from one of [`PRESETS`], as seen from the camera. False for an unknown id.
pub fn apply_preset(scene: &mut Scene3d, id: &str) -> bool {
    let Some((_, _, uv)) = PRESETS.iter().find(|p| p.0 == id) else { return false };
    // Any canvas size gives the same direction for a spot on the disc.
    let d = from_disc(&View::new(&scene.camera, 1000, 1000), Vec2::from_array(*uv));
    set_direction(scene, d);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_doc::Body3d;

    fn scene() -> Scene3d {
        let mut s = Scene3d { figures: vec![crate::new_figure(Body3d::Male, None)], ..Scene3d::default() };
        crate::frame(&mut s, 800, 1000, 0.8);
        s
    }

    #[test]
    fn the_sun_lands_where_it_was_dropped() {
        for fixed in [false, true] {
            let mut s = scene();
            s.look.light_fixed = fixed;
            s.camera.yaw = 0.7;
            let (c, r) = disc(&s, 800, 1000);
            for off in [[0.3, -0.4], [-0.9, 0.1], [0.0, -1.5], [1.2, 0.6]] {
                let p = [c.x + off[0] * r, c.y + off[1] * r];
                let d = from_screen(&s, 800, 1000, p);
                set_direction(&mut s, d);
                let (h, behind) = handle(&s, 800, 1000);
                assert!((h[0] - p[0]).abs() < 2.0 && (h[1] - p[1]).abs() < 2.0, "{off:?}: {h:?} vs {p:?}");
                assert_eq!(behind, Vec2::from_array(off).length() > 1.0, "{off:?}");
            }
        }
    }

    #[test]
    fn upper_left_lights_from_above_and_the_left() {
        let mut s = scene();
        assert!(apply_preset(&mut s, "upperLeft"));
        let view = View::new(&s.camera, 800, 1000);
        let (right, up, fwd) = view.axes();
        let d = direction(&s);
        assert!(d.dot(right) < -0.3 && d.dot(up) > 0.3 && d.dot(-fwd) > 0.2, "{d:?}");
        assert!(!apply_preset(&mut s, "nowhere"));
    }

    #[test]
    fn a_camera_light_turns_with_the_camera_and_a_fixed_one_stays() {
        let mut s = scene();
        let before = direction(&s);
        s.camera.yaw += 1.0;
        assert!(direction(&s).distance(before) > 0.3, "follows the camera");
        set_fixed(&mut s, true);
        let fixed = direction(&s);
        s.camera.yaw -= 2.0;
        assert!(direction(&s).distance(fixed) < 1e-4, "stays in the scene");
        set_fixed(&mut s, false);
        assert!(direction(&s).distance(fixed) < 1e-4, "switching keeps the light where it is");
    }
}
