//! Pencil shading swatches, for judging a pencil's paper grain by eye: hatching at four pressures,
//! then shading with the pencil leaning over, on white. Writes a BMP.
//! `cargo run --release -p photocraft-paint --example pencil_swatch -- out.bmp ["2B Pencil"] [old]`
//! `old` swaps in the pencils' previous grain (a 128 px Paper tile at 50 %, Multiply) to compare.
use photocraft_color::PixelFormat;
use photocraft_geom::Rect;
use photocraft_paint::brush::{MaskMode, Pattern, PatternStyle, Texture};
use photocraft_paint::{StrokePoint, StrokeRenderer, presets};
use photocraft_raster::Surface;

const CELL: i32 = 300;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or_else(|| "pencil.bmp".into());
    let name = args.get(2).cloned().unwrap_or_else(|| "2B Pencil".into());
    let all = presets::builtin();
    let mut brush = presets::find(&all, &name).expect("a pencil preset").brush.clone();
    brush.color = [0.08, 0.08, 0.09, 1.0];
    if args.get(3).is_some_and(|a| a == "old") {
        brush.texture = Texture {
            enabled: true,
            pattern: Pattern::Procedural { style: PatternStyle::Paper, size: 128, seed: 22 },
            mode: MaskMode::Multiply,
            depth: 0.75,
            scale: 0.5,
            ..Default::default()
        };
    }
    // Tuning overrides: TEX_MODE (height, linear, multiply), TEX_DEPTH, TEX_CONTRAST.
    let env = |k: &str| std::env::var(k).ok();
    if let Some(m) = env("TEX_MODE") {
        brush.texture.mode = match m.as_str() {
            "linear" => MaskMode::LinearHeight,
            "multiply" => MaskMode::Multiply,
            "subtract" => MaskMode::Subtract,
            _ => MaskMode::Height,
        };
    }
    if let Some(d) = env("TEX_DEPTH").and_then(|v| v.parse().ok()) {
        brush.texture.depth = d;
    }
    if let Some(c) = env("TEX_CONTRAST").and_then(|v| v.parse().ok()) {
        brush.texture.contrast = c;
    }
    if let Some(i) = env("TEX_INVERT") {
        brush.texture.invert = i == "1";
    }
    if let Some(path) = env("DUMP_PATTERN") {
        let p = photocraft_paint::procedural::pattern(PatternStyle::Tooth, 1024, 7);
        let mut s = Surface::new(PixelFormat::RGBA8);
        s.fill_rect(Rect::new(0, 0, 400, 400), &[1.0, 1.0, 1.0, 1.0]);
        for y in 0..400 {
            for x in 0..400 {
                let v = p.data[y * 1024 + x];
                s.fill_rect(Rect::new(x as i32, y as i32, x as i32 + 1, y as i32 + 1), &[v, v, v, 1.0]);
            }
        }
        write_bmp(&path, &s, 400, 400);
    }
    let fmt = PixelFormat::RGBA8;
    // Hatching at 25, 50, 75 and 100 % pressure, then leaning 50° and 70° at 60 %.
    let cells: [(f32, f32); 6] = [(0.25, 0.0), (0.5, 0.0), (0.75, 0.0), (1.0, 0.0), (0.6, 50.0), (0.6, 70.0)];
    let (w, h) = (CELL * cells.len() as i32, CELL);
    let mut canvas = Surface::new(fmt);
    canvas.fill_rect(Rect::new(0, 0, w, h), &[1.0, 1.0, 1.0, 1.0]);
    for (i, &(pressure, tilt)) in cells.iter().enumerate() {
        let x0 = f64::from(i as i32 * CELL + 30);
        let (y0, y1) = (40.0, f64::from(CELL - 40));
        // Strokes of three back-and-forths each, 4 px apart, as a hand hatches.
        let mut x = x0;
        let mut t = 0.0;
        while x < x0 + 200.0 {
            let mut pts = Vec::new();
            for k in 0..6 {
                let (ya, yb) = if k % 2 == 0 { (y1, y0) } else { (y0, y1) };
                let xa = x + 40.0 * if k % 2 == 0 { 0.0 } else { 1.0 };
                for j in 0..=60 {
                    let f = f64::from(j) / 60.0;
                    let xb = x + 40.0 * if k % 2 == 0 { 1.0 } else { 0.0 };
                    t += 4.0;
                    pts.push(StrokePoint { x: xa + (xb - xa) * f, y: ya + (yb - ya) * f, pressure, tilt_x: tilt, time: t, ..Default::default() });
                }
                x += 4.0;
            }
            let mut r = StrokeRenderer::new(&brush, Some(fmt), 1.0);
            for p in pts.chunks(4) {
                r.push(p);
            }
            let before = canvas.clone();
            r.composite(&before, &mut canvas, None, false, false);
        }
    }
    write_bmp(&out, &canvas, w, h);
    println!("wrote {out} ({w}x{h})");
}

fn write_bmp(path: &str, s: &Surface, w: i32, h: i32) {
    let row = (w as usize * 3).div_ceil(4) * 4;
    let size = 54 + row * h as usize;
    let mut b = Vec::with_capacity(size);
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&(size as u32).to_le_bytes());
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&54u32.to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&w.to_le_bytes());
    b.extend_from_slice(&h.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&24u16.to_le_bytes());
    b.extend_from_slice(&[0; 24]);
    for y in (0..h).rev() {
        let start = b.len();
        for x in 0..w {
            let p = s.pixel(x, y);
            for c in [2, 1, 0] {
                b.push((p[c].clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
        b.resize(start + row, 0);
    }
    std::fs::write(path, b).expect("write the BMP");
}
