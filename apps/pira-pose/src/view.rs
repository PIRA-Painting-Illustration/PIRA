//! The figures' image on the canvas: PIRA's CPU renderer, as in a 3D layer. While dragging, a
//! quick low-resolution preview each step (as PIRA's Object tool does); otherwise the smooth
//! render, made on a background thread with the preview shown until it's ready.

use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions};
use photocraft_doc::Scene3d;
use photocraft_geom::Rect;
use photocraft_scene3d::{self as s3d, Quality};
use std::sync::mpsc;

/// Most pixels a drag preview renders for the whole canvas (PIRA's figure).
const PREVIEW_PIXELS: u64 = 350_000;

/// A rendered image and where it goes on the canvas (canvas pixels).
pub struct Shown {
    pub texture: TextureHandle,
    pub rect: Rect,
}

#[derive(Clone, PartialEq)]
struct Key {
    scene: Vec<u8>,
    size: (u32, u32),
}

struct Job {
    key: Key,
    rx: mpsc::Receiver<(ColorImage, Rect)>,
}

#[derive(Default)]
pub struct View {
    shown: Option<Shown>,
    /// What `shown` is (`final_`: the smooth render rather than a preview).
    key: Option<Key>,
    final_: bool,
    job: Option<Job>,
}

fn key(scene: &Scene3d, size: (u32, u32)) -> Key {
    Key { scene: serde_json::to_vec(scene).unwrap_or_default(), size }
}

impl View {
    /// The image to draw for `scene` on a `size` canvas. `dragging`: previews only.
    pub fn image(&mut self, ctx: &egui::Context, scene: &Scene3d, size: (u32, u32), dragging: bool) -> Option<&Shown> {
        let k = key(scene, size);
        // A finished smooth render.
        if let Some(job) = &self.job
            && let Ok((img, rect)) = job.rx.try_recv()
        {
            let done = job.key.clone();
            self.job = None;
            if done == k {
                self.set(ctx, img, rect, done, true);
            }
        }
        let current = self.key.as_ref() == Some(&k);
        if !current {
            let p = s3d::preview(scene, size.0, size.1, PREVIEW_PIXELS);
            let img = if p.rgba.is_empty() { ColorImage::new([1, 1], vec![Color32::TRANSPARENT]) } else { ColorImage::from_rgba_unmultiplied(p.size, &p.rgba) };
            self.set(ctx, img, p.rect, k.clone(), false);
        }
        if !dragging && !self.final_ && self.job.as_ref().is_none_or(|j| j.key != k) {
            let (tx, rx) = mpsc::channel();
            let scene = scene.clone();
            let c = ctx.clone();
            std::thread::spawn(move || {
                let r = s3d::render::render(&scene, size.0, size.1, Quality::Final);
                let _ = tx.send((photocraft_scene3d_egui::rendered_rect_image(&r), r.rect));
                c.request_repaint();
            });
            self.job = Some(Job { key: k, rx });
        }
        self.shown.as_ref()
    }

    fn set(&mut self, ctx: &egui::Context, img: ColorImage, rect: Rect, k: Key, final_: bool) {
        match &mut self.shown {
            Some(s) => {
                s.texture.set(img, TextureOptions::LINEAR);
                s.rect = rect;
            }
            None => self.shown = Some(Shown { texture: ctx.load_texture("figures", img, TextureOptions::LINEAR), rect }),
        }
        self.key = Some(k);
        self.final_ = final_;
    }

    /// True while a smooth render is still on its way.
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }
}

/// The figures (and the photo under them, if given) as straight-alpha RGBA bytes at canvas size.
pub fn flatten(scene: &Scene3d, size: (u32, u32), photo: Option<&[u8]>) -> Vec<u8> {
    let (w, h) = (size.0 as usize, size.1 as usize);
    let mut out = match photo {
        Some(p) if p.len() == w * h * 4 => p.to_vec(),
        _ => vec![0u8; w * h * 4],
    };
    let r = s3d::render::render(scene, size.0, size.1, Quality::Final);
    let rw = r.rect.width() as usize;
    for (k, p) in r.rgba.iter().enumerate() {
        let (x, y) = (r.rect.x0 as usize + k % rw.max(1), r.rect.y0 as usize + k / rw.max(1));
        if x >= w || y >= h || p[3] <= 0.0 {
            continue;
        }
        let o = &mut out[(y * w + x) * 4..(y * w + x) * 4 + 4];
        let a = p[3].clamp(0.0, 1.0);
        let da = f32::from(o[3]) / 255.0;
        let oa = a + da * (1.0 - a);
        for c in 0..3 {
            let v = (p[c].clamp(0.0, 1.0) * a + f32::from(o[c]) / 255.0 * da * (1.0 - a)) / oa.max(1e-6);
            o[c] = (v * 255.0 + 0.5) as u8;
        }
        o[3] = (oa * 255.0 + 0.5) as u8;
    }
    out
}
