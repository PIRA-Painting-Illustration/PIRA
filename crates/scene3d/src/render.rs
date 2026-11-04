//! The CPU renderer: rasterizes the figures into the canvas-sized image of a 3D layer.
//!
//! Only the figures' bounding box is rendered. Triangles are binned into horizontal bands that
//! render in parallel; each sample is shaded as it wins the depth test (the mannequin has little
//! overdraw), then outlines grow from silhouette and depth edges and the samples are averaged
//! into pixels.

use crate::camera::View;
use crate::mannequin::MAT_BALL;
use crate::{figure_joints, model};
use glam::{Vec2, Vec3};
use photocraft_color::PixelFormat;
use photocraft_doc::{Look3d, Scene3d, Shading3d};
use photocraft_geom::Rect;
use photocraft_raster::Surface;

/// How carefully to render.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    /// One sample per pixel: for live previews while dragging.
    Draft,
    /// Four samples per pixel (smooth edges), when the canvas allows it.
    Final,
}

/// The rendered image: straight-alpha sRGB pixels covering `rect` of the canvas.
#[derive(Clone, Debug, Default)]
pub struct Rendered {
    pub rect: Rect,
    pub rgba: Vec<[f32; 4]>,
}

/// Most samples one render allocates (8 bytes each, plus 1–3 for shadows and outlines).
const MAX_SAMPLES: u64 = 16_000_000;
const BAND: usize = 32;
const SHADOW_ALPHA: f32 = 0.28;

#[derive(Clone, Copy)]
struct Sample {
    depth: f32,
    rgb: [u8; 3],
    /// Figure index + 1; 0 = empty.
    id: u8,
}

const EMPTY: Sample = Sample { depth: f32::INFINITY, rgb: [0; 3], id: 0 };

struct Tri {
    p: [Vec2; 3],
    inv_w: [f32; 3],
    depth: [f32; 3],
    n: [Vec3; 3],
    ball: bool,
    id: u8,
    y0: usize,
    y1: usize,
}

/// Sample grid over the rendered box: sample (sx, sy) sits at canvas pixel
/// `origin + (sx + 0.5, sy + 0.5) / ss * div`.
#[derive(Clone, Copy)]
struct Grid {
    origin: Vec2,
    /// Samples per pixel along each axis (1 or 2).
    ss: usize,
    /// Pixels per sample along each axis when the box is too big for one sample per pixel.
    div: usize,
    w: usize,
    h: usize,
}

impl Grid {
    fn sample_at(&self, px: Vec2) -> Vec2 {
        (px - self.origin) * self.ss as f32 / self.div as f32
    }
}

struct Lights {
    key: Vec3,
    /// The main light's colour times its strength (linear).
    tint: Vec3,
    fill: Vec3,
    view: Vec3,
}

/// Renders `scene` for a canvas of `width` × `height` pixels.
pub fn render(scene: &Scene3d, width: u32, height: u32, quality: Quality) -> Rendered {
    let view = View::new(&scene.camera, width, height);
    let canvas = Rect::new(0, 0, width.min(i32::MAX as u32) as i32, height.min(i32::MAX as u32) as i32);
    let look = &scene.look;
    let lights = lights(scene, &view);
    let outline = if look.outline_px.is_finite() { look.outline_px.clamp(0.0, 64.0) } else { 0.0 };

    // Project every figure (and its floor shadow).
    let mut tris_px: Vec<(Tri, bool)> = Vec::new();
    let mut lo = Vec2::splat(f32::MAX);
    let mut hi = Vec2::splat(f32::MIN);
    for (fi, fig) in scene.figures.iter().enumerate().take(250) {
        if !fig.visible {
            continue;
        }
        let id = (fi + 1) as u8;
        let (skel, mesh) = model(fig.body);
        let joints = figure_joints(skel, fig);
        let floor_y = crate::finite_or(fig.position[1], 0.0);
        let verts: Vec<Option<(crate::camera::Projected, Vec3, Option<Vec2>)>> = mesh
            .vertices
            .iter()
            .map(|v| {
                let m = joints.get(v.joint as usize)?;
                let world = m.transform_point3(Vec3::from_array(v.pos));
                let n = m.transform_vector3(Vec3::from_array(v.normal)).normalize_or(Vec3::Y);
                let p = view.project(world)?;
                let shadow = look.floor_shadow.then(|| shadow_point(world, floor_y, lights.key)).flatten().and_then(|s| view.project(s)).map(|s| s.px);
                Some((p, n, shadow))
            })
            .collect();
        for t in mesh.indices.as_chunks::<3>().0 {
            let [Some(a), Some(b), Some(c)] = [t[0], t[1], t[2]].map(|i| verts.get(i as usize).copied().flatten()) else { continue };
            let ball = mesh.vertices.get(t[0] as usize).is_some_and(|v| v.material == MAT_BALL);
            for q in [a.0.px, b.0.px, c.0.px] {
                lo = lo.min(q);
                hi = hi.max(q);
            }
            tris_px.push((
                Tri {
                    p: [a.0.px, b.0.px, c.0.px],
                    inv_w: [a.0.inv_w, b.0.inv_w, c.0.inv_w],
                    depth: [a.0.depth * a.0.inv_w, b.0.depth * b.0.inv_w, c.0.depth * c.0.inv_w],
                    n: [a.1, b.1, c.1],
                    ball,
                    id,
                    y0: 0,
                    y1: 0,
                },
                false,
            ));
            if let (Some(sa), Some(sb), Some(sc)) = (a.2, b.2, c.2) {
                for q in [sa, sb, sc] {
                    lo = lo.min(q);
                    hi = hi.max(q);
                }
                tris_px.push((Tri { p: [sa, sb, sc], inv_w: [1.0; 3], depth: [0.0; 3], n: [Vec3::Y; 3], ball: false, id, y0: 0, y1: 0 }, true));
            }
        }
    }
    if tris_px.is_empty() || !lo.is_finite() || !hi.is_finite() {
        return Rendered::default();
    }
    let pad = outline.ceil() as i32 + 2;
    let bbox = Rect::new(
        (lo.x.floor() as i32).saturating_sub(pad),
        (lo.y.floor() as i32).saturating_sub(pad),
        (hi.x.ceil() as i32).saturating_add(pad),
        (hi.y.ceil() as i32).saturating_add(pad),
    )
    .intersect(&canvas);
    if bbox.width() == 0 || bbox.height() == 0 {
        return Rendered::default();
    }
    let pixels = bbox.width() as u64 * bbox.height() as u64;
    let mut ss = if quality == Quality::Final { 2usize } else { 1 };
    if pixels * (ss * ss) as u64 > MAX_SAMPLES {
        ss = 1;
    }
    let mut div = 1usize;
    while pixels / (div * div) as u64 > MAX_SAMPLES {
        div += 1;
    }
    let w = (bbox.width() as usize * ss).div_ceil(div);
    let h = (bbox.height() as usize * ss).div_ceil(div);
    let grid = Grid { origin: Vec2::new(bbox.x0 as f32, bbox.y0 as f32), ss, div, w, h };

    // Into sample space, binned by band.
    let bands = h.div_ceil(BAND);
    let mut solid: Vec<Vec<usize>> = vec![Vec::new(); bands];
    let mut shadow: Vec<Vec<usize>> = vec![Vec::new(); bands];
    let mut tris: Vec<Tri> = Vec::with_capacity(tris_px.len());
    for (mut t, is_shadow) in tris_px {
        t.p = t.p.map(|p| grid.sample_at(p));
        let ymin = t.p.iter().fold(f32::MAX, |m, p| m.min(p.y)).floor().max(0.0);
        let ymax = t.p.iter().fold(f32::MIN, |m, p| m.max(p.y)).ceil().min(h as f32);
        if ymax <= ymin {
            continue;
        }
        t.y0 = ymin as usize;
        t.y1 = ymax as usize;
        let k = tris.len();
        let into = if is_shadow { &mut shadow } else { &mut solid };
        for b in (t.y0 / BAND)..=((t.y1.saturating_sub(1)) / BAND).min(bands - 1) {
            if let Some(v) = into.get_mut(b) {
                v.push(k);
            }
        }
        tris.push(t);
    }

    let mut samples = vec![EMPTY; w * h];
    let mut shade_mask = if look.floor_shadow { vec![0u8; w * h] } else { Vec::new() };
    let tris = &tris;
    let lights = &lights;
    for_each_band(&mut samples, w * BAND, |b, chunk| {
        for &k in solid.get(b).map_or(&[][..], |v| v) {
            if let Some(t) = tris.get(k) {
                raster_solid(t, chunk, w, b * BAND, look, lights);
            }
        }
    });
    if look.floor_shadow {
        for_each_band(&mut shade_mask, w * BAND, |b, chunk| {
            for &k in shadow.get(b).map_or(&[][..], |v| v) {
                if let Some(t) = tris.get(k) {
                    raster_cover(t, chunk, w, b * BAND);
                }
            }
        });
    }
    let ring = if outline > 0.0 { outline_mask(&samples, w, h, outline * ss as f32 / div as f32) } else { Vec::new() };
    resolve(bbox, &grid, &samples, &shade_mask, &ring, look)
}

/// [`render`] as layer pixels in `format`, covering at most `canvas`.
pub fn render_surface(scene: &Scene3d, canvas: Rect, format: PixelFormat, quality: Quality) -> Surface {
    let mut s = Surface::new(format);
    let (w, h) = (canvas.x1.max(0) as u32, canvas.y1.max(0) as u32);
    let r = render(scene, w, h, quality);
    if r.rgba.is_empty() {
        return s;
    }
    let ch = format.channels();
    let mut vals = vec![0.0f32; r.rgba.len() * ch];
    for (p, o) in r.rgba.iter().zip(vals.chunks_exact_mut(ch)) {
        if p[3] > 0.0 {
            photocraft_raster::from_rgba_into(&format, *p, o);
        }
    }
    s.write_region(r.rect, &vals);
    s.prune();
    s
}

fn lights(scene: &Scene3d, view: &View) -> Lights {
    let look = &scene.look;
    let key = crate::light::direction(scene);
    let fill = Vec3::new(-key.x, 0.25, -key.z).normalize_or(Vec3::NEG_Z);
    let (_, _, fwd) = view.axes();
    let strength = crate::finite_or(look.light_strength, 1.0).clamp(0.0, 2.0);
    let tint = Vec3::from_array(look.light_color.map(|c| to_linear(crate::finite_or(c, 1.0).clamp(0.0, 1.0)))) * strength;
    Lights { key, tint, fill, view: -fwd }
}

/// Where a point's shadow falls on the floor at height `floor_y`, lit from direction `key`.
fn shadow_point(p: Vec3, floor_y: f32, key: Vec3) -> Option<Vec3> {
    let l = Vec3::new(key.x, key.y.max(0.35), key.z);
    let above = p.y - floor_y;
    (above > -0.05).then(|| p - l * (above.max(0.0) / l.y))
}

#[cfg(not(target_arch = "wasm32"))]
fn for_each_band<T: Send>(data: &mut [T], band_len: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    use rayon::prelude::*;
    data.par_chunks_mut(band_len.max(1)).enumerate().for_each(|(b, c)| f(b, c));
}

#[cfg(target_arch = "wasm32")]
fn for_each_band<T>(data: &mut [T], band_len: usize, f: impl Fn(usize, &mut [T])) {
    data.chunks_mut(band_len.max(1)).enumerate().for_each(|(b, c)| f(b, c));
}

/// Calls `f(x, y, barycentrics)` for each sample centre of `band` (rows `y_base..`) inside `t`.
fn scan(t: &Tri, w: usize, rows: usize, y_base: usize, mut f: impl FnMut(usize, usize, [f32; 3])) {
    let [a, b, c] = t.p;
    let area = (b - a).perp_dot(c - a);
    if area.abs() < 1e-9 || !area.is_finite() {
        return;
    }
    let inv = 1.0 / area;
    let y_lo = t.y0.max(y_base);
    let y_hi = t.y1.min(y_base + rows);
    let xmin = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
    let xmax = (a.x.max(b.x).max(c.x).ceil().max(0.0) as usize).min(w);
    for y in y_lo..y_hi {
        let py = y as f32 + 0.5;
        for x in xmin..xmax {
            let p = Vec2::new(x as f32 + 0.5, py);
            let w0 = (c - b).perp_dot(p - b) * inv;
            let w1 = (a - c).perp_dot(p - c) * inv;
            let w2 = 1.0 - w0 - w1;
            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                f(x, y - y_base, [w0, w1, w2]);
            }
        }
    }
}

fn raster_solid(t: &Tri, band: &mut [Sample], w: usize, y_base: usize, look: &Look3d, lights: &Lights) {
    let rows = band.len() / w.max(1);
    scan(t, w, rows, y_base, |x, y, bw| {
        let pw = [bw[0] * t.inv_w[0], bw[1] * t.inv_w[1], bw[2] * t.inv_w[2]];
        let sum = pw[0] + pw[1] + pw[2];
        if sum <= 0.0 {
            return;
        }
        let depth = (bw[0] * t.depth[0] + bw[1] * t.depth[1] + bw[2] * t.depth[2]) / sum;
        let Some(s) = band.get_mut(y * w + x) else { return };
        if depth >= s.depth || depth <= 0.0 {
            return;
        }
        let n = (t.n[0] * pw[0] + t.n[1] * pw[1] + t.n[2] * pw[2]).normalize_or(Vec3::Y);
        *s = Sample { depth, rgb: shade(look, lights, n, t.ball), id: t.id };
    });
}

fn raster_cover(t: &Tri, band: &mut [u8], w: usize, y_base: usize) {
    let rows = band.len() / w.max(1);
    scan(t, w, rows, y_base, |x, y, _| {
        if let Some(s) = band.get_mut(y * w + x) {
            *s = 1;
        }
    });
}

fn to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn to_srgb8(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5) as u8
}

fn color(c: [f32; 3]) -> Vec3 {
    Vec3::from_array(c.map(|v| crate::finite_or(v, 0.5).clamp(0.0, 1.0)))
}

/// The colour of a surface with normal `n` (sRGB bytes).
fn shade(look: &Look3d, l: &Lights, n: Vec3, ball: bool) -> [u8; 3] {
    let n = if n.dot(l.view) < 0.0 { -n } else { n };
    let mut base = color(look.body_color);
    if ball {
        base *= 0.78;
    }
    let ambient = crate::finite_or(look.ambient, 0.35).clamp(0.0, 1.0);
    let lin = base.to_array().map(to_linear);
    let lin = Vec3::from_array(lin);
    let out = match look.shading {
        Shading3d::Silhouette => return color(look.outline_color).to_array().map(|c| (c * 255.0 + 0.5) as u8),
        Shading3d::Flat => return base.to_array().map(|c| (c * 255.0 + 0.5) as u8),
        Shading3d::Shaded => {
            let key = l.tint * (n.dot(l.key).max(0.0) * 0.9);
            let fill = n.dot(l.fill).max(0.0) * 0.25;
            let sky = Vec3::new(0.16, 0.14, 0.13).lerp(Vec3::new(0.32, 0.34, 0.4), n.y * 0.5 + 0.5) * (ambient / 0.35);
            let h = (l.key + l.view).normalize_or(Vec3::Y);
            let spec = l.tint * (n.dot(h).max(0.0).powf(40.0) * 0.18);
            let edge = 1.0 - n.dot(l.view).max(0.0);
            let rim = edge.powi(3) * 0.12;
            // A light from behind catches the silhouette edges on its side (rim light).
            let behind = (-l.key.dot(l.view)).max(0.0);
            let back_rim = l.tint * (edge.powf(1.5) * n.dot(l.key).max(0.0) * behind * 1.4);
            lin * (key + Vec3::splat(fill) + sky) + spec + back_rim + Vec3::splat(rim)
        }
        Shading3d::Toon => {
            let d = n.dot(l.key);
            let tone = if d > 0.3 {
                1.0
            } else if d > -0.15 {
                0.62
            } else {
                0.4
            };
            // The lit bands take the light's colour and strength; the darkest stays ambient.
            let lit = Vec3::splat(1.0).lerp(l.tint, if d > -0.15 { 1.0 } else { 0.0 });
            lin * lit * (tone * (1.0 - ambient * 0.5) + ambient * 0.5)
        }
    };
    out.to_array().map(to_srgb8)
}

/// Samples within `radius` of a silhouette or depth edge (1 = outline).
fn outline_mask(s: &[Sample], w: usize, h: usize, radius: f32) -> Vec<u8> {
    // Edge seeds: a filled sample next to an empty one, another figure, or a surface well behind.
    let far = |a: &Sample, b: &Sample| b.id == 0 || b.id != a.id || b.depth - a.depth > 0.06 + a.depth * 0.01;
    let mut dist = vec![u16::MAX; w * h];
    for y in 0..h {
        for x in 0..w {
            let Some(a) = s.get(y * w + x) else { continue };
            if a.id == 0 {
                continue;
            }
            let edge = (x == 0 || s.get(y * w + x - 1).is_none_or(|b| far(a, b)))
                || (x + 1 >= w || s.get(y * w + x + 1).is_none_or(|b| far(a, b)))
                || (y == 0 || s.get((y - 1) * w + x).is_none_or(|b| far(a, b)))
                || (y + 1 >= h || s.get((y + 1) * w + x).is_none_or(|b| far(a, b)));
            if edge && let Some(d) = dist.get_mut(y * w + x) {
                *d = 0;
            }
        }
    }
    // Chamfer distance (3 per straight step, 4 diagonal), two passes.
    let step = |d: &mut [u16], i: usize, j: Option<usize>, cost: u16| {
        if let Some(j) = j
            && let (Some(&dj), Some(&di)) = (d.get(j), d.get(i))
        {
            let v = dj.saturating_add(cost);
            if v < di
                && let Some(t) = d.get_mut(i)
            {
                *t = v;
            }
        }
    };
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            step(&mut dist, i, (x > 0).then(|| i - 1), 3);
            step(&mut dist, i, (y > 0).then(|| i - w), 3);
            step(&mut dist, i, (x > 0 && y > 0).then(|| i - w - 1), 4);
            step(&mut dist, i, (x + 1 < w && y > 0).then(|| i - w + 1), 4);
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            step(&mut dist, i, (x + 1 < w).then(|| i + 1), 3);
            step(&mut dist, i, (y + 1 < h).then(|| i + w), 3);
            step(&mut dist, i, (x + 1 < w && y + 1 < h).then(|| i + w + 1), 4);
            step(&mut dist, i, (x > 0 && y + 1 < h).then(|| i + w - 1), 4);
        }
    }
    let limit = (radius.max(0.5) * 3.0).min(u16::MAX as f32 - 1.0) as u16;
    dist.into_iter().map(|d| u8::from(d <= limit)).collect()
}

/// Averages the samples into straight-alpha pixels.
fn resolve(bbox: Rect, g: &Grid, s: &[Sample], shadow: &[u8], ring: &[u8], look: &Look3d) -> Rendered {
    let (pw, ph) = (bbox.width() as usize, bbox.height() as usize);
    let oc = color(look.outline_color);
    let sample = |i: usize| -> [f32; 4] {
        if ring.get(i).copied() == Some(1) {
            return [oc.x, oc.y, oc.z, 1.0];
        }
        match s.get(i) {
            Some(p) if p.id != 0 => [p.rgb[0] as f32 / 255.0, p.rgb[1] as f32 / 255.0, p.rgb[2] as f32 / 255.0, 1.0],
            _ if shadow.get(i).copied() == Some(1) => [0.0, 0.0, 0.0, SHADOW_ALPHA],
            _ => [0.0; 4],
        }
    };
    let mut rgba = vec![[0.0f32; 4]; pw * ph];
    for (y, row) in rgba.chunks_exact_mut(pw.max(1)).enumerate() {
        for (x, out) in row.iter_mut().enumerate() {
            let mut acc = [0.0f32; 4];
            let n = if g.div > 1 {
                let i = (y / g.div) * g.w + x / g.div;
                let p = sample(i);
                acc = [p[0] * p[3], p[1] * p[3], p[2] * p[3], p[3]];
                1.0
            } else {
                for dy in 0..g.ss {
                    for dx in 0..g.ss {
                        let i = (y * g.ss + dy) * g.w + x * g.ss + dx;
                        let p = sample(i);
                        for c in 0..3 {
                            acc[c] += p[c] * p[3];
                        }
                        acc[3] += p[3];
                    }
                }
                (g.ss * g.ss) as f32
            };
            let a = acc[3] / n;
            if a > 0.0 {
                *out = [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3], a];
            }
        }
    }
    let _ = g.h;
    Rendered { rect: bbox, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_doc::Figure3d;

    fn scene() -> Scene3d {
        Scene3d { figures: vec![Figure3d::default()], ..Scene3d::default() }
    }

    #[test]
    fn a_figure_renders_mid_canvas_with_transparent_corners() {
        let r = render(&scene(), 400, 600, Quality::Draft);
        assert!(r.rect.width() > 20 && r.rect.height() > 100, "{:?}", r.rect);
        let opaque = r.rgba.iter().filter(|p| p[3] > 0.99).count();
        assert!(opaque > 2000, "{opaque} opaque pixels");
        assert_eq!(r.rgba.first().map(|p| p[3]), Some(0.0));
    }

    #[test]
    fn final_quality_smooths_edges() {
        let r = render(&scene(), 400, 600, Quality::Final);
        assert!(r.rgba.iter().any(|p| p[3] > 0.2 && p[3] < 0.8));
    }

    #[test]
    fn outlines_and_every_shading_render() {
        for shading in Shading3d::ALL {
            let mut s = scene();
            s.look.shading = shading;
            s.look.outline_px = 3.0;
            let r = render(&s, 300, 400, Quality::Draft);
            let dark = r.rgba.iter().filter(|p| p[3] > 0.99 && p[0] < 0.1).count();
            assert!(dark > 100, "{shading:?}: {dark} outline pixels");
        }
    }

    #[test]
    fn an_empty_scene_or_a_camera_facing_away_renders_nothing() {
        assert!(render(&Scene3d::default(), 300, 300, Quality::Final).rgba.is_empty());
        let mut s = scene();
        // Behind the camera: it sits at z = -45.8 looking further down -Z.
        s.camera.target = [0.0, 0.9, -50.0];
        s.camera.yaw = 0.0;
        assert!(render(&s, 300, 300, Quality::Draft).rgba.is_empty());
    }

    #[test]
    fn a_huge_canvas_stays_within_the_sample_budget() {
        let mut s = scene();
        s.camera.distance = 1.0;
        let r = render(&s, 30_000, 30_000, Quality::Final);
        assert!(!r.rgba.is_empty());
    }

    #[test]
    fn surfaces_come_in_the_documents_format() {
        for fmt in [PixelFormat::RGBA8, PixelFormat::RGBA16, PixelFormat::CMYKA8, PixelFormat::GRAYA8] {
            let s = render_surface(&scene(), Rect::new(0, 0, 200, 300), fmt, Quality::Draft);
            assert_eq!(s.format(), fmt);
            assert!(!s.content_bounds().is_empty());
        }
    }
}
