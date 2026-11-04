//! Renders a sample 3D layer to a PPM file over grey: `cargo run --release -p photocraft-scene3d --example render -- out.ppm [pose] [shading] [outline] [light preset] [strength] [r,g,b]`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use photocraft_doc::{Body3d, Scene3d, Shading3d};
use photocraft_scene3d::{Quality, frame, new_figure, render::render};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or_else(|| "out.ppm".into());
    let pose = args.get(2).cloned().unwrap_or_else(|| "Contrapposto".into());
    let mut s = Scene3d { figures: vec![new_figure(Body3d::Male, Some(&pose))], ..Scene3d::default() };
    let mut f = new_figure(Body3d::Female, Some("Walking"));
    f.position = [0.8, 0.0, -0.3];
    s.figures.push(f);
    if let Some(sh) = args.get(3).and_then(|v| Shading3d::from_id(v)) {
        s.look.shading = sh;
    }
    s.look.outline_px = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    if let Some(id) = args.get(5) {
        assert!(photocraft_scene3d::light::apply_preset(&mut s, id), "unknown light preset {id}");
    }
    if let Some(v) = args.get(6).and_then(|v| v.parse().ok()) {
        s.look.light_strength = v;
    }
    if let Some(c) = args.get(7) {
        let v: Vec<f32> = c.split(',').filter_map(|x| x.parse().ok()).collect();
        s.look.light_color = [v[0], v[1], v[2]];
    }
    let (w, h) = (900u32, 1200u32);
    frame(&mut s, w, h, 0.85);
    let t = std::time::Instant::now();
    let r = render(&s, w, h, Quality::Final);
    eprintln!("rendered {:?} in {:?}", r.rect, t.elapsed());
    let mut img = vec![200u8; (w * h * 3) as usize];
    let (rw, rh) = (r.rect.width() as usize, r.rect.height() as usize);
    for y in 0..rh {
        for x in 0..rw {
            let p = r.rgba[y * rw + x];
            let i = ((y + r.rect.y0 as usize) * w as usize + x + r.rect.x0 as usize) * 3;
            for c in 0..3 {
                img[i + c] = ((p[c] * p[3] + 200.0 / 255.0 * (1.0 - p[3])) * 255.0) as u8;
            }
        }
    }
    let mut data = format!("P6 {w} {h} 255\n").into_bytes();
    data.extend(img);
    std::fs::write(out, data).unwrap();
}
