//! The 3D layer's camera: an orbit camera whose image is the whole canvas.

use glam::{Mat4, Vec2, Vec3, Vec4};
use photocraft_doc::Camera3d;

const NEAR: f32 = 0.05;
const FAR: f32 = 200.0;

/// A [`Camera3d`] set up for one canvas size.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub size: Vec2,
    pub eye: Vec3,
    pub view: Mat4,
    pub proj: Mat4,
    pub view_proj: Mat4,
    pub offset: Vec2,
    pub parallel: bool,
}

/// Where a point lands on the canvas.
#[derive(Clone, Copy, Debug)]
pub struct Projected {
    /// Canvas pixels.
    pub px: Vec2,
    /// Distance in front of the camera (metres).
    pub depth: f32,
    /// 1 / w (1 for a parallel camera), for perspective-correct interpolation.
    pub inv_w: f32,
}

fn sane(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { fallback }
}

fn vec3(a: [f32; 3]) -> Vec3 {
    let v = Vec3::from_array(a);
    if v.is_finite() { v.clamp(Vec3::splat(-1000.0), Vec3::splat(1000.0)) } else { Vec3::ZERO }
}

/// The camera position for `cam`.
pub fn eye(cam: &Camera3d) -> Vec3 {
    let (yaw, pitch) = (sane(cam.yaw, -1e4, 1e4, 0.0), sane(cam.pitch, -1.55, 1.55, 0.0));
    let dir = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos());
    vec3(cam.target) + dir * sane(cam.distance, 0.1, 500.0, 4.0)
}

impl View {
    pub fn new(cam: &Camera3d, width: u32, height: u32) -> Self {
        let size = Vec2::new(width.max(1) as f32, height.max(1) as f32);
        let target = vec3(cam.target);
        let eye = eye(cam);
        let view = look_at(eye, target, Vec3::Y);
        let fov = sane(cam.fov_y_deg, 5.0, 120.0, 35.0).to_radians();
        let aspect = size.x / size.y;
        let proj = if cam.parallel {
            let half_h = sane(cam.distance, 0.1, 500.0, 4.0) * (fov * 0.5).tan();
            orthographic(half_h * aspect, half_h, NEAR, FAR)
        } else {
            perspective(fov, aspect, NEAR, FAR)
        };
        let offset = Vec2::new(sane(cam.offset_px[0], -1e6, 1e6, 0.0), sane(cam.offset_px[1], -1e6, 1e6, 0.0));
        View { size, eye, view, proj, view_proj: proj * view, offset, parallel: cam.parallel }
    }

    /// World point → canvas pixels (None when behind the camera).
    pub fn project(&self, p: Vec3) -> Option<Projected> {
        let c = self.view_proj * p.extend(1.0);
        if c.w <= NEAR * 0.5 {
            return None;
        }
        let n = c.truncate() / c.w;
        let px = Vec2::new((n.x * 0.5 + 0.5) * self.size.x, (0.5 - n.y * 0.5) * self.size.y) + self.offset;
        let depth = -(self.view * p.extend(1.0)).z;
        Some(Projected { px, depth, inv_w: if self.parallel { 1.0 } else { 1.0 / c.w } })
    }

    /// The ray through a canvas pixel: origin and unit direction.
    pub fn ray(&self, px: Vec2) -> (Vec3, Vec3) {
        let px = px - self.offset;
        let inv = self.view_proj.inverse();
        let ndc = Vec2::new(px.x / self.size.x * 2.0 - 1.0, 1.0 - px.y / self.size.y * 2.0);
        let near = inv.project_point3(ndc.extend(0.0));
        let far = inv.project_point3(ndc.extend(1.0));
        (near, (far - near).normalize_or(Vec3::NEG_Z))
    }

    /// The camera's right and up directions in the world.
    pub fn axes(&self) -> (Vec3, Vec3, Vec3) {
        let v = self.view;
        let right = Vec3::new(v.x_axis.x, v.y_axis.x, v.z_axis.x);
        let up = Vec3::new(v.x_axis.y, v.y_axis.y, v.z_axis.y);
        let back = Vec3::new(v.x_axis.z, v.y_axis.z, v.z_axis.z);
        (right, up, -back)
    }

    /// World units per canvas pixel at a point.
    pub fn world_per_px(&self, p: Vec3) -> f32 {
        let fov_half = (1.0 / self.proj.y_axis.y).max(1e-4);
        if self.parallel {
            2.0 * fov_half / self.size.y
        } else {
            2.0 * (p - self.eye).length() * fov_half / self.size.y
        }
    }
}

/// Turns the camera around its target by a drag of `delta` canvas pixels.
pub fn orbit(cam: &mut Camera3d, delta: Vec2) {
    cam.yaw = sane(cam.yaw - delta.x * 0.006, -1e4, 1e4, 0.0);
    cam.pitch = sane(cam.pitch + delta.y * 0.006, -1.5, 1.5, 0.0);
}

/// Slides the camera (and its target) by a drag of `delta` canvas pixels.
pub fn pan(cam: &mut Camera3d, delta: Vec2, view: &View) {
    let (right, up, _) = view.axes();
    let per_px = view.world_per_px(vec3(cam.target));
    let t = vec3(cam.target) + (-right * delta.x + up * delta.y) * per_px;
    cam.target = t.to_array();
}

/// Moves the camera nearer (positive `steps`) or farther.
pub fn dolly(cam: &mut Camera3d, factor: f32) {
    cam.distance = sane(cam.distance / sane(factor, 0.05, 20.0, 1.0), 0.3, 200.0, 4.0);
}

/// Right-handed view matrix (the camera looks down its −Z).
fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    let f = (target - eye).normalize_or(Vec3::NEG_Z);
    let s = f.cross(up).normalize_or(Vec3::X);
    let u = s.cross(f);
    Mat4::from_cols(
        Vec4::new(s.x, u.x, -f.x, 0.0),
        Vec4::new(s.y, u.y, -f.y, 0.0),
        Vec4::new(s.z, u.z, -f.z, 0.0),
        Vec4::new(-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
    )
}

/// Right-handed perspective with depth 0..1.
fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let h = 1.0 / (fov_y * 0.5).tan();
    let r = far / (near - far);
    Mat4::from_cols(
        Vec4::new(h / aspect, 0.0, 0.0, 0.0),
        Vec4::new(0.0, h, 0.0, 0.0),
        Vec4::new(0.0, 0.0, r, -1.0),
        Vec4::new(0.0, 0.0, r * near, 0.0),
    )
}

/// Right-handed parallel projection with depth 0..1.
fn orthographic(half_w: f32, half_h: f32, near: f32, far: f32) -> Mat4 {
    let r = 1.0 / (near - far);
    Mat4::from_cols(
        Vec4::new(1.0 / half_w, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0 / half_h, 0.0, 0.0),
        Vec4::new(0.0, 0.0, r, 0.0),
        Vec4::new(0.0, 0.0, r * near, 1.0),
    )
}

/// How far along the ray a capsule is hit, if at all (approximate: the ray's closest approach).
pub fn ray_capsule(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, r: f32) -> Option<f32> {
    let ab = b - a;
    let c = ab.length_squared();
    let (t, s) = if c < 1e-10 {
        ((a - origin).dot(dir), 0.0)
    } else {
        let w = origin - a;
        let b_ = dir.dot(ab);
        let d = dir.dot(w);
        let e = ab.dot(w);
        let denom = c - b_ * b_;
        let s = if denom.abs() > 1e-8 { ((e - b_ * d) / denom).clamp(0.0, 1.0) } else { 0.0 };
        let t = (b_ * s - d).max(0.0);
        (t, ((origin + dir * t - a).dot(ab) / c).clamp(0.0, 1.0))
    };
    let p = origin + dir * t;
    let q = a + ab * s;
    let dist = (p - q).length();
    (t > 0.0 && dist < r).then(|| t - (r * r - dist * dist).max(0.0).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_lands_mid_canvas_and_the_ray_returns_to_it() {
        for parallel in [false, true] {
            let cam = Camera3d { parallel, ..Camera3d::default() };
            let v = View::new(&cam, 800, 600);
            let p = v.project(Vec3::from_array(cam.target)).unwrap();
            assert!((p.px - Vec2::new(400.0, 300.0)).length() < 0.5, "{parallel}: {:?}", p.px);
            assert!((p.depth - cam.distance).abs() < 1e-3);
            let (o, d) = v.ray(Vec2::new(400.0, 300.0));
            let t = (Vec3::from_array(cam.target) - o).dot(d);
            assert!((o + d * t - Vec3::from_array(cam.target)).length() < 0.01);
        }
    }

    #[test]
    fn the_offset_slides_the_image() {
        let cam = Camera3d { offset_px: [30.0, -10.0], ..Camera3d::default() };
        let p = View::new(&cam, 800, 600).project(Vec3::from_array(cam.target)).unwrap();
        assert!((p.px - Vec2::new(430.0, 290.0)).length() < 0.5);
    }

    #[test]
    fn hostile_numbers_still_give_a_finite_view() {
        let cam = Camera3d { yaw: f32::NAN, pitch: f32::INFINITY, distance: -3.0, fov_y_deg: 0.0, target: [f32::NAN; 3], ..Camera3d::default() };
        let v = View::new(&cam, 0, 0);
        assert!(v.view_proj.is_finite());
    }
}
