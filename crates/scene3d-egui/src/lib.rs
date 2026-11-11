//! Posing 3D figures on a canvas with a pen or mouse: what a drag does, the handles under the
//! pen, and the overlays drawn over the figures. Shared by PIRA's Object tool
//! (`ui-egui/src/tools/object_tool.rs`) and PIRA Pose (`apps/pira-pose`), so a change here
//! reaches both.
//!
//! Everything works in canvas pixels; the caller maps them to the screen (`to_screen`).
//!
//! * A drag on a body part swings it about its joint in the canvas plane ([`DragKind::Swing`]);
//!   with Twist it turns about its own bone. A drag on the hips slides the figure across its floor
//!   (with Twist, turns it). A drag on empty canvas orbits the camera.
//! * Joints: dots on the head, elbows, wrists, knees and ankles; dragging one puts that joint
//!   under the pen ([`pose::reach`]).
//! * Match photo: the photo's joints as coral dots, to drag onto the person's joints.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use egui::{Color32, Pos2, Stroke, Vec2};
use photocraft_doc::{Body3d, Scene3d};
use photocraft_scene3d::glam;
use photocraft_scene3d::{self as s3d, camera, light, pose};

/// What a drag does to the scene.
#[derive(Clone, Debug, PartialEq)]
pub enum DragKind {
    /// Swing part (figure, joint) about its joint.
    Swing(usize, usize),
    /// Turn part (figure, joint) about its own bone.
    Twist(usize, usize),
    /// Slide figure across its floor.
    Slide(usize),
    /// Turn figure about the vertical.
    Turn(usize),
    Orbit,
    Pan,
    Dolly,
    /// Place the main light: the sun goes under the pen.
    Light,
    /// A photo-match joint (its landmark index) being dragged onto the photo's joint.
    PhotoPoint(usize),
    /// A figure's joint (figure, joint) being dragged to a spot (Joints).
    Reach(usize, usize),
}

/// What a plain drag starting at `at` does: the part under it (figure, joint) and how it moves,
/// or an orbit over empty canvas. `twist` turns parts about their bones instead of swinging them.
pub fn pose_drag_at(scene: &Scene3d, (w, h): (u32, u32), at: [f32; 2], twist: bool) -> (Option<(usize, usize)>, DragKind) {
    match pose::pick(scene, w, h, at) {
        Some(hit) => {
            let kind = match (hit.joint, twist) {
                (0, false) => DragKind::Slide(hit.figure),
                (0, true) => DragKind::Turn(hit.figure),
                (j, false) => DragKind::Swing(hit.figure, j),
                (j, true) => DragKind::Twist(hit.figure, j),
            };
            (Some((hit.figure, hit.joint)), kind)
        }
        None => (None, DragKind::Orbit),
    }
}

/// Applies one step of a drag from `a` to `b` (canvas pixels) to the scene. True when it changed.
/// ([`DragKind::PhotoPoint`] changes nothing here: the caller refits the match on release.)
pub fn step(scene: &mut Scene3d, kind: &DragKind, (w, h): (u32, u32), a: [f32; 2], b: [f32; 2]) -> bool {
    let delta = glam::Vec2::new(b[0] - a[0], b[1] - a[1]);
    match *kind {
        DragKind::Swing(f, j) => pose::swing(scene, w, h, f, j, a, b),
        DragKind::Twist(f, j) => scene.figures.get_mut(f).is_some_and(|fig| pose::twist(fig, j, delta.x * 0.012)),
        DragKind::Slide(f) => pose::slide(scene, w, h, f, a, b),
        DragKind::Turn(f) => match scene.figures.get_mut(f) {
            Some(fig) => {
                fig.yaw += delta.x * 0.012;
                true
            }
            None => false,
        },
        DragKind::Orbit => {
            camera::orbit(&mut scene.camera, delta);
            true
        }
        DragKind::Pan => {
            let view = camera::View::new(&scene.camera, w, h);
            camera::pan(&mut scene.camera, delta, &view);
            true
        }
        DragKind::Dolly => {
            camera::dolly(&mut scene.camera, (-delta.y * 0.006).exp());
            true
        }
        DragKind::Light => {
            let dir = light::from_screen(scene, w, h, b);
            light::set_direction(scene, dir);
            true
        }
        DragKind::PhotoPoint(_) => false,
        DragKind::Reach(f, j) => pose::reach(scene, w, h, f, j, b),
    }
}

/// The figure joint (Joints) within `reach` of canvas point `at`, nearest first.
pub fn joint_handle_at(scene: &Scene3d, (w, h): (u32, u32), at: [f32; 2], reach: f32) -> Option<(usize, usize)> {
    let at = Pos2::new(at[0], at[1]);
    let mut best: Option<((usize, usize), f32)> = None;
    for (fi, fig) in scene.figures.iter().enumerate().filter(|(_, f)| f.visible) {
        for (name, _) in pose::REACH_HANDLES {
            let Some(j) = pose::joint_index(fig, name) else { continue };
            let Some((p, _)) = pose::joint_px(scene, w, h, fi, j) else { continue };
            let d = Pos2::new(p[0], p[1]).distance(at);
            if d <= reach && best.is_none_or(|(_, b)| d < b) {
                best = Some(((fi, j), d));
            }
        }
    }
    best.map(|(hit, _)| hit)
}

/// The photo-match joint (a landmark index) within `reach` of canvas point `at`, nearest first.
pub fn photo_handle_at(points: &[glam::Vec2], at: [f32; 2], reach: f32) -> Option<usize> {
    let at = glam::Vec2::new(at[0], at[1]);
    s3d::retarget::HANDLES
        .iter()
        .filter_map(|&i| points.get(i).map(|p| (i, p.distance(at))))
        .filter(|(_, d)| *d <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The joints that get a small white dot in Pose mode.
pub const DOT_JOINTS: [&str; 15] = [
    "hips",
    "chest",
    "head",
    "upper_arm.L",
    "upper_arm.R",
    "forearm.L",
    "forearm.R",
    "hand.L",
    "hand.R",
    "thigh.L",
    "thigh.R",
    "shin.L",
    "shin.R",
    "foot.L",
    "foot.R",
];

/// Coral: the draggable handles (Joints, Match photo).
pub const CORAL: Color32 = Color32::from_rgb(255, 122, 92);

/// A small white dot on each visible figure's main joints.
pub fn draw_joint_dots(painter: &egui::Painter, scene: &Scene3d, (w, h): (u32, u32), to_screen: &dyn Fn([f32; 2]) -> Pos2) {
    for (fi, fig) in scene.figures.iter().enumerate() {
        if !fig.visible {
            continue;
        }
        for name in DOT_JOINTS {
            let Some(j) = pose::joint_index(fig, name) else { continue };
            if let Some((pivot, _)) = pose::joint_px(scene, w, h, fi, j) {
                let c = to_screen(pivot);
                painter.circle_filled(c, 3.5, Color32::from_black_alpha(110));
                painter.circle_filled(c, 2.5, Color32::from_white_alpha(200));
            }
        }
    }
}

/// Joints: the main joints as coral dots on a thin skeleton; `held` (figure, joint) is drawn larger.
pub fn draw_reach_handles(painter: &egui::Painter, scene: &Scene3d, (w, h): (u32, u32), to_screen: &dyn Fn([f32; 2]) -> Pos2, held: Option<(usize, usize)>) {
    for (fi, fig) in scene.figures.iter().enumerate().filter(|(_, f)| f.visible) {
        let at = |name: &str| pose::joint_index(fig, name).and_then(|j| pose::joint_px(scene, w, h, fi, j)).map(|(p, _)| to_screen(p));
        for (a, b) in pose::REACH_LINES {
            if let (Some(a), Some(b)) = (at(a), at(b)) {
                painter.line_segment([a, b], Stroke::new(3.0, Color32::from_black_alpha(90)));
                painter.line_segment([a, b], Stroke::new(1.5, CORAL));
            }
        }
        for (name, _) in pose::REACH_HANDLES {
            let Some(c) = at(name) else { continue };
            let r = if held == pose::joint_index(fig, name).map(|j| (fi, j)) { 9.0 } else { 6.5 };
            painter.circle_filled(c, r + 1.5, Color32::from_black_alpha(120));
            painter.circle_filled(c, r, CORAL);
        }
    }
}

/// The selected part: its bone, a ring round its pivot and its name.
pub fn draw_selected(
    painter: &egui::Painter,
    scene: &Scene3d,
    (w, h): (u32, u32),
    to_screen: &dyn Fn([f32; 2]) -> Pos2,
    (fi, j): (usize, usize),
    (accent, text): (Color32, Color32),
) {
    let Some((pivot, tip)) = pose::joint_px(scene, w, h, fi, j) else { return };
    let (a, b) = (to_screen(pivot), to_screen(tip));
    painter.line_segment([a, b], Stroke::new(3.0, accent));
    let r = (b - a).length().max(14.0);
    painter.circle_stroke(a, r, Stroke::new(1.5, accent.gamma_multiply(0.7)));
    painter.circle_filled(a, 5.0, accent);
    if let Some(name) = scene.figures.get(fi).and_then(|f| pose::joint_name(f, j)) {
        painter.text(a + Vec2::new(10.0, -10.0), egui::Align2::LEFT_BOTTOM, pose::joint_label(name), egui::FontId::proportional(13.0), text);
    }
}

/// Match photo: the photo's joints as the match read them (canvas pixels), to drag where they
/// belong. `moved(i)`: the user has placed joint `i` (drawn white); `held`: being dragged.
pub fn draw_photo_points(
    painter: &egui::Painter,
    points: &[glam::Vec2],
    moved: &dyn Fn(usize) -> bool,
    held: Option<usize>,
    to_screen: &dyn Fn([f32; 2]) -> Pos2,
) {
    let at = |i: usize| points.get(i).map(|p| to_screen([p.x, p.y]));
    for (a, b) in s3d::retarget::HANDLE_LINES {
        if let (Some(a), Some(b)) = (at(a), at(b)) {
            painter.line_segment([a, b], Stroke::new(3.0, Color32::from_black_alpha(90)));
            painter.line_segment([a, b], Stroke::new(1.5, CORAL));
        }
    }
    for i in s3d::retarget::HANDLES {
        let Some(c) = at(i) else { continue };
        let fixed = moved(i);
        let r = if held == Some(i) { 9.0 } else { 6.5 };
        painter.circle_filled(c, r + 1.5, Color32::from_black_alpha(120));
        painter.circle_filled(c, r, if fixed { Color32::WHITE } else { CORAL });
        if fixed {
            painter.circle_stroke(c, r - 1.0, Stroke::new(2.0, CORAL));
        }
    }
}

/// The light disc (front within the inner ring, behind out to the outer one) and the sun on it.
pub fn draw_light(painter: &egui::Painter, scene: &Scene3d, (w, h): (u32, u32), to_screen: &dyn Fn([f32; 2]) -> Pos2) {
    let (c, r) = light::disc(scene, w, h);
    let centre = to_screen(c.to_array());
    let radius = (to_screen([c.x + r, c.y]) - centre).length();
    let shade = Color32::from_black_alpha(90);
    let sun_color = Color32::from_rgb(255, 200, 60);
    painter.circle_stroke(centre, radius, Stroke::new(3.0, shade));
    painter.circle_stroke(centre, radius, Stroke::new(1.5, Color32::from_white_alpha(170)));
    painter.circle_stroke(centre, radius * 2.0, Stroke::new(1.0, Color32::from_white_alpha(70)));
    let (p, behind) = light::handle(scene, w, h);
    let sun = to_screen(p);
    painter.line_segment([centre, sun], Stroke::new(1.5, Color32::from_white_alpha(150)));
    painter.circle_filled(centre, 3.0, Color32::from_white_alpha(170));
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::TAU / 8.0;
        let d = Vec2::new(a.cos(), a.sin());
        painter.line_segment([sun + d * 12.0, sun + d * 17.0], Stroke::new(2.0, sun_color));
    }
    painter.circle_filled(sun, 10.0, shade);
    if behind {
        painter.circle_stroke(sun, 8.0, Stroke::new(2.5, sun_color));
    } else {
        painter.circle_filled(sun, 8.0, sun_color);
    }
}

/// Light colour swatches: hex, name.
pub const LIGHT_SWATCHES: [(&str, &str); 7] = [
    ("#ffffff", "White"),
    ("#ffe2b8", "Warm"),
    ("#ffb36b", "Sunset"),
    ("#ff8a5c", "Firelight"),
    ("#d6e6ff", "Cool"),
    ("#9fb8ff", "Moonlight"),
    ("#c9a0ff", "Neon violet"),
];

/// An sRGB colour (0–1) as `#rrggbb`.
pub fn hex(c: [f32; 3]) -> String {
    let b = c.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
    format!("#{:02x}{:02x}{:02x}", b[0], b[1], b[2])
}

/// The pose library's thumbnail size.
pub const THUMB: (u32, u32) = (96, 128);

/// A pose library thumbnail: a male figure in pose `id`, framed.
pub fn thumb_image(id: &str) -> egui::ColorImage {
    let (w, h) = THUMB;
    let mut scene = Scene3d { figures: vec![s3d::new_figure(Body3d::Male, Some(id))], ..Scene3d::default() };
    scene.look.floor_shadow = false;
    s3d::frame(&mut scene, w, h, 0.92);
    let r = s3d::render::render(&scene, w, h, s3d::Quality::Final);
    rendered_image(&r, (w, h))
}

/// A render as an egui image covering the whole `w` × `h` canvas (transparent round the figures).
pub fn rendered_image(r: &s3d::Rendered, (w, h): (u32, u32)) -> egui::ColorImage {
    let mut px = vec![Color32::TRANSPARENT; (w * h) as usize];
    let rw = r.rect.width() as usize;
    for (k, p) in r.rgba.iter().enumerate() {
        let (x, y) = (r.rect.x0 as usize + k % rw.max(1), r.rect.y0 as usize + k / rw.max(1));
        if x < w as usize
            && let Some(o) = px.get_mut(y * w as usize + x)
        {
            *o = color(*p);
        }
    }
    egui::ColorImage::new([w as usize, h as usize], px)
}

/// Just the render's own rectangle (`r.rect`) as an egui image.
pub fn rendered_rect_image(r: &s3d::Rendered) -> egui::ColorImage {
    let (w, h) = (r.rect.width() as usize, r.rect.height() as usize);
    if w == 0 || h == 0 || r.rgba.len() < w * h {
        return egui::ColorImage::new([1, 1], vec![Color32::TRANSPARENT]);
    }
    egui::ColorImage::new([w, h], r.rgba.iter().map(|p| color(*p)).collect())
}

fn color(p: [f32; 4]) -> Color32 {
    let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    Color32::from_rgba_unmultiplied(c(p[0]), c(p[1]), c(p[2]), c(p[3]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> Scene3d {
        let mut s = Scene3d { figures: vec![s3d::new_figure(Body3d::Male, None)], ..Scene3d::default() };
        s3d::frame(&mut s, 900, 1200, 0.82);
        s
    }

    #[test]
    fn a_drag_on_a_forearm_swings_it_and_empty_canvas_orbits() {
        let mut s = scene();
        let fig = &s.figures[0];
        let j = pose::joint_index(fig, "forearm.L").unwrap();
        let (pivot, tip) = pose::joint_px(&s, 900, 1200, 0, j).unwrap();
        let p = [(pivot[0] + tip[0]) / 2.0, (pivot[1] + tip[1]) / 2.0];
        let (hit, kind) = pose_drag_at(&s, (900, 1200), p, false);
        assert_eq!(hit.map(|h| h.0), Some(0));
        assert!(matches!(kind, DragKind::Swing(0, _)), "{kind:?}");
        let before = s.clone();
        assert!(step(&mut s, &kind, (900, 1200), p, [p[0] + 40.0, p[1] - 80.0]));
        assert_ne!(s.figures[0], before.figures[0]);
        let (none, orbit) = pose_drag_at(&s, (900, 1200), [3.0, 3.0], false);
        assert_eq!((none, orbit.clone()), (None, DragKind::Orbit));
        let yaw = s.camera.yaw;
        step(&mut s, &orbit, (900, 1200), [3.0, 3.0], [103.0, 3.0]);
        assert_ne!(s.camera.yaw, yaw);
    }

    #[test]
    fn joint_handles_are_found_near_their_joints() {
        let s = scene();
        let fig = &s.figures[0];
        let j = pose::joint_index(fig, "hand.L").unwrap();
        let (p, _) = pose::joint_px(&s, 900, 1200, 0, j).unwrap();
        assert_eq!(joint_handle_at(&s, (900, 1200), [p[0] + 3.0, p[1]], 20.0), Some((0, j)));
        assert_eq!(joint_handle_at(&s, (900, 1200), [1.0, 1.0], 20.0), None);
    }

    #[test]
    fn a_thumbnail_shows_the_figure() {
        let img = thumb_image("Standing");
        assert_eq!(img.size, [THUMB.0 as usize, THUMB.1 as usize]);
        assert!(img.pixels.iter().any(|p| p.a() > 200));
        assert!(img.pixels.iter().any(|p| p.a() == 0));
        assert_eq!(hex([1.0, 0.0, 0.5]), "#ff0080");
    }
}
