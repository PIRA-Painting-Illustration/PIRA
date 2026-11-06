//! Comic pages: panels (Layer › Comic › New Panels, `comic.newPanels`) and the Panel Cut tool
//! (`comic.cutPanel`), speech balloons (`comic.newBalloon`), effect lines (`comic.effectLines`)
//! and screentones (`comic.newTone`).
//!
//! A panel is a group marked as one (`PiraLayer::panel`), with a vector mask in the panel's shape
//! (so what's drawn inside stays inside) holding the panel's art layer and, on top, its border: a
//! shape layer with an inside stroke along the same outline. Panels are convex polygons; cutting
//! one along a line leaves two, a gutter apart.

use photocraft_doc::vector::{Path, ShapeLayer, ShapeStroke, StrokeAlign, Subpath, VectorMask};
use photocraft_doc::{Document, Layer, LayerContent, LayerId};
use serde_json::{Value, json};

use crate::commands::CommandSpec;
use crate::{EngineError, Result, Session};

/// A panel or balloon outline: its corners in order.
pub type Outline = Vec<[f64; 2]>;

const NEW_PANELS: &str = "comic.newPanels";
const CUT_PANEL: &str = "comic.cutPanel";
const NEW_BALLOON: &str = "comic.newBalloon";
const EFFECT_LINES: &str = "comic.effectLines";
const NEW_TONE: &str = "comic.newTone";
/// Most panels one layout makes.
const MAX_GRID: u64 = 8;

fn bad(cmd: &str, msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

/// The default border width and gutter for a `w` × `h` page (pixels).
pub fn defaults(w: u32, h: u32) -> (f64, f64) {
    let side = f64::from(w.min(h));
    ((side * 0.0025).round().max(2.0), (side * 0.015).round().max(4.0))
}

/// Where a new panel layout goes: the comic page's safe area, or the canvas less 5 % margins.
fn layout_area(doc: &Document) -> [f64; 4] {
    let (w, h) = (doc.size.width, doc.size.height);
    match doc.pira.page {
        Some(page) => page.safe_area(w, h),
        None => {
            let m = f64::from(w.min(h)) * 0.05;
            [m, m, f64::from(w) - m, f64::from(h) - m]
        }
    }
}

fn number(cmd: &str, p: &Value, key: &str, default: f64, range: std::ops::RangeInclusive<f64>) -> Result<f64> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_f64()
            .filter(|n| n.is_finite() && range.contains(n))
            .ok_or_else(|| bad(cmd, format!("`{key}` must be a number in {}..={}", range.start(), range.end()))),
    }
}

/// A closed polygon path through `pts`.
fn polygon_path(pts: &[[f64; 2]]) -> Path {
    Path::new(vec![Subpath::polygon(&pts.iter().map(|p| (p[0], p[1])).collect::<Vec<_>>())])
}

/// The anchors of a panel's outline (its vector mask's first subpath).
pub fn panel_outline(l: &Layer) -> Option<Vec<[f64; 2]>> {
    let sub = l.vector_mask.as_ref()?.path.subpaths.first()?;
    Some(sub.knots.iter().map(|k| [k.anchor.x, k.anchor.y]).collect())
}

/// A panel's border shape layer, rendered for `doc`.
fn border_layer(doc: &Document, pts: &[[f64; 2]], width: f64) -> Layer {
    let stroke = ShapeStroke { width: width as f32, align: StrokeAlign::Inside, ..ShapeStroke::default() };
    let mut sh = ShapeLayer { path: polygon_path(pts), fill: None, stroke: Some(stroke), ..ShapeLayer::default() };
    crate::vector_cmds::refresh_shape(doc, &mut sh);
    Layer::new("Border", LayerContent::Shape(sh))
}

/// A panel group: its art layer and border inside a vector mask of its outline.
fn panel_group(doc: &Document, name: String, pts: &[[f64; 2]], width: f64) -> Layer {
    let art = Layer::raster(format!("{name} art"), doc.pixel_format());
    let border = border_layer(doc, pts, width);
    let mut g = Layer::group(name, vec![art, border]);
    g.vector_mask = Some(VectorMask::new(polygon_path(pts)));
    g.pira.panel = true;
    g
}

/// The border width of a panel group (its top shape layer's stroke).
fn border_width(l: &Layer) -> Option<f64> {
    l.children()?.iter().rev().find_map(|c| match &c.content {
        LayerContent::Shape(sh) => sh.stroke.as_ref().map(|s| f64::from(s.width)),
        _ => None,
    })
}

/// Layer › Comic › New Panels: a `rows` × `columns` grid of panels over the page's safe area,
/// `gutter` apart, with `border`-wide ink borders, in a "Panels" group.
fn new_panels(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let (w, h) = (st.doc.size.width, st.doc.size.height);
    let (border, gutter) = defaults(w, h);
    let rows = number(NEW_PANELS, p, "rows", 1.0, 1.0..=MAX_GRID as f64)?.round() as usize;
    let cols = number(NEW_PANELS, p, "columns", 1.0, 1.0..=MAX_GRID as f64)?.round() as usize;
    let gutter = number(NEW_PANELS, p, "gutter", gutter, 0.0..=f64::from(w.min(h)) / 4.0)?;
    let border = number(NEW_PANELS, p, "border", border, 0.0..=200.0)?;
    let a = layout_area(&st.doc);
    let (cw, ch) = ((a[2] - a[0] - gutter * (cols - 1) as f64) / cols as f64, (a[3] - a[1] - gutter * (rows - 1) as f64) / rows as f64);
    if cw < 8.0 || ch < 8.0 {
        return Err(bad(NEW_PANELS, "the panels would be too small: use fewer rows, columns or a smaller gutter"));
    }
    let doc = st.doc.clone();
    let mut panels = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let (x0, y0) = (a[0] + c as f64 * (cw + gutter), a[1] + r as f64 * (ch + gutter));
            let pts = [[x0, y0], [x0 + cw, y0], [x0 + cw, y0 + ch], [x0, y0 + ch]];
            panels.push(panel_group(&doc, format!("Panel {}", r * cols + c + 1), &pts, border));
        }
    }
    // Reading order top to bottom in the Layers panel: the first panel on top.
    panels.reverse();
    let group = Layer::group("Panels", panels);
    let id = group.id;
    s.edit("New Panels", |doc, _| {
        doc.layers.push(group);
        Ok(())
    })?;
    s.select_layer(id)?;
    Ok(json!({"group": id.0, "panels": rows * cols}))
}

/// Points of `poly` on the side `n · p >= d`, cut along that line (Sutherland–Hodgman).
fn clip(poly: &[[f64; 2]], n: [f64; 2], d: f64) -> Vec<[f64; 2]> {
    let side = |p: [f64; 2]| n[0] * p[0] + n[1] * p[1] - d;
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (sa, sb) = (side(a), side(b));
        if sa >= 0.0 {
            out.push(a);
        }
        if (sa >= 0.0) != (sb >= 0.0) {
            let t = sa / (sa - sb);
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    out
}

fn area(poly: &[[f64; 2]]) -> f64 {
    (0..poly.len())
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        .abs()
        / 2.0
}

fn contains(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + poly.len() - 1) % poly.len()]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
    }
    inside
}

/// Split `poly` along the line through `from` and `to`, each side pulled back by half the
/// gutter. `None` when the line misses the panel or leaves a sliver.
pub fn split(poly: &[[f64; 2]], from: [f64; 2], to: [f64; 2], gutter: f64) -> Option<(Outline, Outline)> {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let len = dx.hypot(dy);
    if len < 1e-6 {
        return None;
    }
    let n = [-dy / len, dx / len];
    let c = n[0] * from[0] + n[1] * from[1];
    let a = clip(poly, n, c + gutter / 2.0);
    let b = clip(poly, [-n[0], -n[1]], -(c - gutter / 2.0));
    (a.len() >= 3 && b.len() >= 3 && area(&a) >= 64.0 && area(&b) >= 64.0).then_some((a, b))
}

/// The id of the panel group under `at`, topmost first.
fn panel_at(doc: &Document, at: [f64; 2]) -> Option<LayerId> {
    doc.walk().into_iter().rev().find(|(_, _, l)| l.pira.panel && l.visible && panel_outline(l).is_some_and(|o| contains(&o, at))).map(|(_, _, l)| l.id)
}

/// The Panel Cut tool: split the panel the line crosses (`layer`, or the one under the line's
/// middle) along the line from `from` to `to`, leaving `gutter` between the two halves. The
/// art stays with the first half; the second gets an empty art layer and its own border.
fn cut_panel(s: &mut Session, p: &Value) -> Result<Value> {
    let pt = |k: &str| -> Result<[f64; 2]> {
        let a = p.get(k).and_then(Value::as_array).ok_or_else(|| bad(CUT_PANEL, format!("`{k}` must be [x, y]")))?;
        match (a.first().and_then(Value::as_f64), a.get(1).and_then(Value::as_f64)) {
            (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Ok([x, y]),
            _ => Err(bad(CUT_PANEL, format!("`{k}` must be [x, y]"))),
        }
    };
    let (from, to) = (pt("from")?, pt("to")?);
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let doc = st.doc.clone();
    let id = match crate::commands::explicit_layer(s, p)? {
        Some(id) => id,
        None => panel_at(&doc, [(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0]).ok_or_else(|| bad(CUT_PANEL, "draw the cut across a panel"))?,
    };
    let panel = doc.layer(id).ok_or(EngineError::NoLayer(id))?;
    if !panel.pira.panel {
        return Err(bad(CUT_PANEL, "that layer isn't a panel"));
    }
    let outline = panel_outline(panel).ok_or_else(|| bad(CUT_PANEL, "the panel has no outline"))?;
    let (_, default_gutter) = defaults(doc.size.width, doc.size.height);
    let gutter = number(CUT_PANEL, p, "gutter", default_gutter, 0.0..=f64::from(doc.size.width.min(doc.size.height)) / 4.0)?;
    let (a, b) = split(&outline, from, to, gutter).ok_or_else(|| bad(CUT_PANEL, "the cut must cross the panel and leave room on both sides"))?;
    let width = border_width(panel).unwrap_or_else(|| defaults(doc.size.width, doc.size.height).0);
    let border_a = border_layer(&doc, &a, width);
    let count = doc.walk().iter().filter(|(_, _, l)| l.pira.panel).count();
    let second = panel_group(&doc, format!("Panel {}", count + 1), &b, width);
    let second_id = second.id;
    let (path, _, _) = doc.walk().into_iter().find(|(_, _, l)| l.id == id).ok_or(EngineError::NoLayer(id))?;
    s.edit("Cut Panel", |doc, _| {
        let l = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
        l.vector_mask = Some(VectorMask::new(polygon_path(&a)));
        if let Some(children) = l.children_mut() {
            match children.iter_mut().rev().find(|c| matches!(c.content, LayerContent::Shape(_))) {
                Some(border) => border.content = border_a.content.clone(),
                None => children.push(border_a.clone()),
            }
        }
        // The new panel goes just above the one cut.
        let (parent, i) = path.split_at(path.len() - 1);
        let siblings =
            if parent.is_empty() { &mut doc.layers } else { doc.layer_at_mut(parent).and_then(Layer::children_mut).ok_or(EngineError::NoLayer(id))? };
        siblings.insert(i[0] + 1, second.clone());
        Ok(())
    })?;
    s.select_layer(second_id)?;
    Ok(json!({"panels": [id.0, second_id.0]}))
}

fn point(cmd: &str, p: &Value, key: &str) -> Result<Option<[f64; 2]>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let a = v.as_array();
            match (a.and_then(|a| a.first()).and_then(Value::as_f64), a.and_then(|a| a.get(1)).and_then(Value::as_f64)) {
                (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Ok(Some([x, y])),
                _ => Err(bad(cmd, format!("`{key}` must be [x, y]"))),
            }
        }
    }
}

/// A small repeatable random sequence (effect lines and burst balloons look hand-drawn, but the
/// same params always give the same drawing).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// The selection's bounds `[x0, y0, x1, y1]`, if there is a selection.
fn selection_box(doc: &Document) -> Option<[f64; 4]> {
    let r = doc.selection.as_ref()?.content_bounds();
    (!r.is_empty()).then(|| [f64::from(r.x0), f64::from(r.y0), f64::from(r.x1), f64::from(r.y1)])
}

/// A balloon's outline: `shape` around `c` with half-sizes `(a, b)`, its tail running out to
/// `tail` when there is one.
pub fn balloon_outline(shape: &str, c: [f64; 2], a: f64, b: f64, tail: Option<[f64; 2]>) -> Vec<[f64; 2]> {
    use std::f64::consts::{PI, TAU};
    let mut rng = Rng(0x5eed_ba11_0011);
    // Spikes for a shout, scallops for a thought, else an ellipse; rounded box for "rect".
    let radius = |t: f64, rng: &mut Rng| match shape {
        "burst" => {
            if (t / TAU * 28.0).round() as i64 % 2 == 0 {
                1.12 + rng.next() * 0.08
            } else {
                0.86
            }
        }
        "cloud" => 1.0 + 0.09 * (t * 9.0).sin().abs(),
        _ => 1.0,
    };
    let at = |t: f64, k: f64| -> [f64; 2] {
        if shape == "rect" {
            // A rounded rectangle: a superellipse.
            let (ct, st) = (t.cos(), t.sin());
            let e = 0.25;
            [c[0] + a * k * ct.signum() * ct.abs().powf(e), c[1] + b * k * st.signum() * st.abs().powf(e)]
        } else {
            [c[0] + a * k * t.cos(), c[1] + b * k * t.sin()]
        }
    };
    let n = if shape == "burst" { 28 } else { 96 };
    let Some(tip) = tail else {
        return (0..n)
            .map(|i| {
                let t = TAU * f64::from(i) / f64::from(n);
                at(t, radius(t, &mut rng))
            })
            .collect();
    };
    // The tail leaves the outline around the angle (in the balloon's own proportions) towards
    // its tip, a fixed share of the outline wide.
    let phi = ((tip[1] - c[1]) / b).atan2((tip[0] - c[0]) / a);
    let half = if shape == "burst" { PI / 14.0 } else { 0.2 };
    let mut out: Vec<[f64; 2]> = (0..=n)
        .map(|i| {
            let t = phi + half + (TAU - 2.0 * half) * f64::from(i) / f64::from(n);
            at(t, radius(t, &mut rng))
        })
        .collect();
    out.push(tip);
    out
}

/// Layer › Comic › New Balloon: a speech balloon (`shape` round, rect, cloud or burst) of `size`
/// around `center`, its tail pointing at `tail`, holding `text` in a paragraph type layer, in a
/// "Balloon" group above the active layer.
fn new_balloon(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let doc = st.doc.clone();
    let (w, h) = (f64::from(doc.size.width), f64::from(doc.size.height));
    let shape = p.get("shape").and_then(Value::as_str).unwrap_or("round");
    if !matches!(shape, "round" | "rect" | "cloud" | "burst") {
        return Err(bad(NEW_BALLOON, "`shape` must be round, rect, cloud or burst"));
    }
    let center = point(NEW_BALLOON, p, "center")?.unwrap_or([w / 2.0, h / 2.0]);
    let size = match p.get("size") {
        None | Some(Value::Null) => [w.min(h) * 0.3, w.min(h) * 0.2],
        Some(_) => {
            let sz = point(NEW_BALLOON, p, "size")?.unwrap_or_default();
            if !(sz[0] >= 16.0 && sz[1] >= 16.0 && sz[0] <= w * 2.0 && sz[1] <= h * 2.0) {
                return Err(bad(NEW_BALLOON, "`size` must be [width, height], each 16 px up to twice the canvas"));
            }
            sz
        }
    };
    let (a, b) = (size[0] / 2.0, size[1] / 2.0);
    let tail = match point(NEW_BALLOON, p, "tail")? {
        Some(t) => Some(t),
        // By default it points down and to the left, past the balloon.
        None if p.get("tail") != Some(&Value::Null) => Some([center[0] - a * 0.5, center[1] + b * 1.7]),
        None => None,
    };
    let border = number(NEW_BALLOON, p, "border", defaults(doc.size.width, doc.size.height).0 * 0.6, 0.0..=100.0)?;
    let outline = balloon_outline(shape, center, a, b, tail.filter(|t| ((t[0] - center[0]) / a).hypot((t[1] - center[1]) / b) > 1.05));
    let stroke = (border > 0.0).then(|| ShapeStroke { width: border as f32, ..ShapeStroke::default() });
    let mut sh =
        ShapeLayer { path: polygon_path(&outline), fill: Some(photocraft_doc::Fill::Solid(photocraft_doc::Color::WHITE)), stroke, ..ShapeLayer::default() };
    crate::vector_cmds::refresh_shape(&doc, &mut sh);
    // The words, centred in the balloon's middle, at a size that suits it.
    let text = p.get("text").and_then(Value::as_str).unwrap_or("…");
    let size_pt = number(NEW_BALLOON, p, "fontSize", (size[1] * 0.13 * 72.0 / f64::from(doc.resolution_dpi.max(1.0))).max(4.0), 1.0..=1296.0)?;
    let (bw, bh) = (size[0] * 0.68, size[1] * 0.62);
    let mut tp = json!({"text": text, "box": [center[0] - bw / 2.0, center[1] - bh / 2.0, bw, bh], "align": "center", "size": size_pt, "color": "#000000"});
    if let Some(o) = p.get("orientation") {
        tp["orientation"] = o.clone();
    }
    let mut t = crate::type_cmds::new_text_layer(s, &tp, NEW_BALLOON)?;
    crate::type_cmds::refresh(&doc, &mut t);
    let words = Layer::new(text.chars().take(24).collect::<String>(), LayerContent::Text(t));
    let mut group = Layer::group("Balloon", vec![Layer::new("Balloon shape", LayerContent::Shape(sh)), words]);
    group.pira.balloon = true;
    let id = s.edit("New Balloon", |doc, active| {
        let id = crate::layer_place::insert_new(doc, *active, group, crate::layer_place::Place::AutoEditable);
        *active = Some(id);
        Ok(id)
    })?;
    Ok(json!({"layer": id.0}))
}

/// Layer › Comic › Focus Lines / Speed Lines: tapered ink lines as a shape layer. Focus lines
/// (`kind` focus) close in on `center` and stop at the edge of an ellipse of radii `inner`
/// (default: the selection, else a third of the canvas); speed lines (`kind` speed) run at
/// `angle` degrees across `area` (default: the selection, else the canvas).
fn effect_lines(s: &mut Session, p: &Value) -> Result<Value> {
    use std::f64::consts::TAU;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let doc = st.doc.clone();
    let (w, h) = (f64::from(doc.size.width), f64::from(doc.size.height));
    let kind = p.get("kind").and_then(Value::as_str).unwrap_or("focus");
    if !matches!(kind, "focus" | "speed") {
        return Err(bad(EFFECT_LINES, "`kind` must be focus or speed"));
    }
    let sel = selection_box(&doc);
    let count = number(EFFECT_LINES, p, "count", if kind == "focus" { 120.0 } else { 60.0 }, 4.0..=600.0)?.round() as usize;
    let seed = number(EFFECT_LINES, p, "seed", 1.0, 0.0..=1e9)? as u64;
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ seed);
    let mut subpaths = Vec::new();
    let tri = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| Subpath::polygon(&[(a[0], a[1]), (b[0], b[1]), (c[0], c[1])]);
    if kind == "focus" {
        let box_ = sel.unwrap_or([w / 3.0, h / 3.0, w * 2.0 / 3.0, h * 2.0 / 3.0]);
        let center = point(EFFECT_LINES, p, "center")?.unwrap_or([(box_[0] + box_[2]) / 2.0, (box_[1] + box_[3]) / 2.0]);
        let inner = point(EFFECT_LINES, p, "inner")?.unwrap_or([(box_[2] - box_[0]) / 2.0, (box_[3] - box_[1]) / 2.0]);
        let reach = w.hypot(h);
        let base = TAU * reach / count as f64 * 0.35;
        for i in 0..count {
            let t = TAU * (i as f64 + rng.next() * 0.6) / count as f64;
            let (dx, dy) = (t.cos(), t.sin());
            let r0 = (inner[0] * dx).hypot(inner[1] * dy) * (1.0 + rng.next() * 0.3);
            let wb = base * (0.5 + rng.next());
            let tip = [center[0] + dx * r0, center[1] + dy * r0];
            let far = [center[0] + dx * reach, center[1] + dy * reach];
            let (px, py) = (-dy * wb / 2.0, dx * wb / 2.0);
            subpaths.push(tri(tip, [far[0] + px, far[1] + py], [far[0] - px, far[1] - py]));
        }
    } else {
        let area = sel.unwrap_or([0.0, 0.0, w, h]);
        let angle = number(EFFECT_LINES, p, "angle", 0.0, -360.0..=360.0)?.to_radians();
        let (dx, dy) = (angle.cos(), angle.sin());
        let (nx, ny) = (-dy, dx);
        let c = [(area[0] + area[2]) / 2.0, (area[1] + area[3]) / 2.0];
        let (aw, ah) = (area[2] - area[0], area[3] - area[1]);
        // Extents of the area along and across the lines.
        let along = (aw * dx).abs() + (ah * dy).abs();
        let across = (aw * nx).abs() + (ah * ny).abs();
        let width = number(EFFECT_LINES, p, "width", (w.min(h) * 0.006).max(2.0), 0.5..=200.0)?;
        for _ in 0..count {
            let off = (rng.next() - 0.5) * across;
            let len = along * (0.25 + rng.next() * 0.6);
            let start = (rng.next() - 0.5) * (along - len);
            let o = [c[0] + nx * off + dx * start, c[1] + ny * off + dy * start];
            let (tail, tip) = ([o[0] - dx * len / 2.0, o[1] - dy * len / 2.0], [o[0] + dx * len / 2.0, o[1] + dy * len / 2.0]);
            let wl = width * (0.5 + rng.next());
            let (px, py) = (nx * wl / 2.0, ny * wl / 2.0);
            subpaths.push(tri([tail[0] + px, tail[1] + py], tip, [tail[0] - px, tail[1] - py]));
        }
    }
    let color = match p.get("color").and_then(Value::as_str) {
        None => photocraft_doc::Color::BLACK,
        Some(hex) => {
            let c = crate::commands::parse_hex(hex).ok_or_else(|| bad(EFFECT_LINES, "`color` must be #rrggbb"))?;
            photocraft_doc::Color::rgba(c[0], c[1], c[2], 1.0)
        }
    };
    let mut sh = ShapeLayer { path: Path::new(subpaths), fill: Some(photocraft_doc::Fill::Solid(color)), ..ShapeLayer::default() };
    crate::vector_cmds::refresh_shape(&doc, &mut sh);
    let name = if kind == "focus" { "Focus Lines" } else { "Speed Lines" };
    let layer = Layer::new(name, LayerContent::Shape(sh));
    let id = s.edit(name, |doc, active| {
        let id = crate::layer_place::insert_new(doc, *active, layer, crate::layer_place::Place::Auto);
        *active = Some(id);
        Ok(id)
    })?;
    Ok(json!({"layer": id.0, "lines": count}))
}

/// The dot tile of a screentone: `lpi` lines per inch at `dpi`, dots covering `density` of it.
/// Several cells make one tile so its size comes out a whole number of pixels.
pub fn tone_tile(dpi: f64, lpi: f64, density: f64) -> (u32, Vec<f32>) {
    let cell = dpi / lpi;
    let n = (1..=12)
        .min_by(|a, b| {
            let err = |k: &i32| ((f64::from(*k) * cell).round() - f64::from(*k) * cell).abs() / f64::from(*k);
            err(a).total_cmp(&err(b))
        })
        .unwrap_or(1);
    let size = (f64::from(n) * cell).round().max(2.0) as u32;
    let step = f64::from(size) / f64::from(n);
    let radius = step * (density / std::f64::consts::PI).sqrt();
    let mut px = vec![0.0f32; (size * size * 4) as usize];
    const SUB: u32 = 4;
    for y in 0..size {
        for x in 0..size {
            let mut cover = 0u32;
            for sy in 0..SUB {
                for sx in 0..SUB {
                    let (fx, fy) = (f64::from(x) + (f64::from(sx) + 0.5) / f64::from(SUB), f64::from(y) + (f64::from(sy) + 0.5) / f64::from(SUB));
                    // Distance to the nearest dot centre on the cell grid.
                    let (cx, cy) = (((fx / step).floor() + 0.5) * step, ((fy / step).floor() + 0.5) * step);
                    if (fx - cx).hypot(fy - cy) <= radius {
                        cover += 1;
                    }
                }
            }
            let i = ((y * size + x) * 4) as usize;
            px[i + 3] = cover as f32 / (SUB * SUB) as f32;
        }
    }
    (size, px)
}

/// Layer › Comic › Screentone: a Pattern Fill layer of black dots (`lpi` lines per inch,
/// `density` percent ink, turned `angle` degrees), masked to the selection as a fill layer is.
fn new_tone(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let dpi = f64::from(st.doc.resolution_dpi.max(1.0));
    let lpi = number(NEW_TONE, p, "lpi", 60.0, 10.0..=150.0)?.round();
    let density = number(NEW_TONE, p, "density", 20.0, 2.0..=70.0)?.round();
    let angle = number(NEW_TONE, p, "angle", 45.0, -360.0..=360.0)?;
    if dpi / lpi < 2.0 {
        return Err(bad(NEW_TONE, "the dots would be under two pixels apart: lower the lines per inch or raise the document's resolution"));
    }
    let (size, px) = tone_tile(dpi, lpi, density / 100.0);
    let mut surface = photocraft_raster::Surface::new(photocraft_color::PixelFormat::RGBA8);
    surface.write_region(photocraft_geom::Rect::new(0, 0, size as i32, size as i32), &px);
    let pattern = photocraft_doc::Pattern {
        id: format!("pira-tone-{}dpi-{lpi}lpi-{density}", dpi.round()),
        name: format!("Screentone {lpi} lpi {density}%"),
        width: size,
        height: size,
        surface,
    };
    let default_mask = crate::commands::layer_panel_option(s, "useDefaultMasksOnFillLayers", true);
    let fp = json!({"angle": angle, "scale": 100});
    let id = s.edit("New Screentone", |doc, active| {
        let id = crate::pattern_cmds::insert_fill_layer(doc, active, &pattern, &fp, default_mask)?;
        if let Some(l) = doc.layer_mut(id) {
            l.name = format!("Tone {density}%");
        }
        Ok(id)
    })?;
    Ok(json!({"layer": id.0, "pattern": pattern.id, "tile": size}))
}

pub fn specs() -> Vec<CommandSpec> {
    let has_doc = |s: &Session| -> std::result::Result<(), String> { s.active().map(|_| ()).ok_or_else(|| "no document open".into()) };
    vec![
        CommandSpec {
            id: NEW_PANELS,
            label: "New Panels",
            menu: &["Layer", "Comic"],
            shortcut: None,
            params: r#"{"rows":1..8=1,"columns":1..8=1,"gutter":px? (default 1.5 % of the shorter side),"border":0..200 px? (default 0.25 %)} (over the comic page's safe area, or the canvas less 5 %; each panel a group with a vector mask, an art layer and an ink border)"#,
            enabled: has_doc,
            run: new_panels,
            journal: true,
        },
        CommandSpec {
            id: CUT_PANEL,
            label: "Cut Panel",
            menu: &[],
            shortcut: None,
            params: r#"{"from":[x,y],"to":[x,y],"gutter":px? (default 1.5 % of the shorter side),"layer":id? (default the panel under the line's middle)}"#,
            enabled: has_doc,
            run: cut_panel,
            journal: true,
        },
        CommandSpec {
            id: NEW_BALLOON,
            label: "New Balloon",
            menu: &["Layer", "Comic"],
            shortcut: None,
            params: r#"{"shape":"round|rect|cloud|burst"="round","center":[x,y]? (default the canvas centre),"size":[w,h]? (default 30 % × 20 % of the shorter side),"tail":[x,y]|null? (the tip; null: no tail; default down and left),"text":str="…","fontSize":pt?,"orientation":"horizontal|vertical"?,"border":0..100 px?}"#,
            enabled: has_doc,
            run: new_balloon,
            journal: true,
        },
        CommandSpec {
            id: EFFECT_LINES,
            label: "Effect Lines",
            menu: &[],
            shortcut: None,
            params: r##"{"kind":"focus|speed"="focus","center":[x,y]?,"inner":[rx,ry]? (focus: the clear middle; default the selection or a third of the canvas),"angle":deg=0 (speed),"width":px? (speed),"count":4..600,"color":"#rrggbb"="#000000","seed":n=1}"##,
            enabled: has_doc,
            run: effect_lines,
            journal: true,
        },
        CommandSpec {
            id: NEW_TONE,
            label: "Screentone",
            menu: &["Layer", "Comic"],
            shortcut: None,
            params: r#"{"density":2..70 %=20,"lpi":10..150=60 (lines per inch),"angle":deg=45} (a Pattern Fill layer of dots, masked to the selection)"#,
            enabled: has_doc,
            run: new_tone,
            journal: true,
        },
    ]
}

#[cfg(test)]
mod tests;
