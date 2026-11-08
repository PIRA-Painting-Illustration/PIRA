//! Special rulers on the canvas and in the options bars (the engine side, and what each kind
//! does, is `ruler_cmds`). With the Ruler tool's Special Ruler option set to a kind, a drag on
//! the canvas draws a ruler of that kind (`ruler.add`); with any kind, a drag on a ruler's
//! handle reshapes it (`ruler.set`). Painting tools show a snap switch while there are rulers;
//! the snapping itself is `stroke_constraint::special`.

use crate::PhotocraftApp;
use crate::canvas::{ToolEvent, ViewXform};
use egui::{Color32, Pos2, Stroke};
use photocraft_doc::{DocId, RulerShape};
use photocraft_engine::ruler_cmds::{Guide, curve_path, guide};
use serde_json::json;

/// Screen points around a handle that grab it.
const GRAB: f32 = 14.0;
/// A curve ruler keeps at most this many of the points its drag passed through.
const CURVE_POINTS: usize = 24;

/// The Special Ruler kinds the options bar offers (0 = none: the Ruler tool measures).
pub const KINDS: [(usize, &str); 7] =
    [(0, "Measure"), (1, "Straight"), (2, "Curve"), (3, "Concentric Circle"), (4, "Parallel Line"), (5, "Radial Line"), (6, "Fisheye")];

/// A special ruler gesture in progress.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpecialRulerState {
    /// A ruler being drawn: its document, kind and the points the drag passed through.
    pub making: Option<(DocId, usize, Vec<[f64; 2]>)>,
    /// A handle being dragged: its document, the ruler's index, the handle and the ruler as dragged.
    pub moving: Option<(DocId, usize, usize, RulerShape)>,
    /// The ruler last drawn or grabbed: what Delete Ruler deletes.
    pub selected: Option<(DocId, usize)>,
}

/// The handles that reshape a ruler.
pub fn handles(shape: &RulerShape) -> Vec<[f64; 2]> {
    match shape {
        RulerShape::Concentric { center, radii, angle } => {
            let (s, c) = angle.sin_cos();
            vec![*center, [center[0] + c * radii[0], center[1] + s * radii[0]], [center[0] - s * radii[1], center[1] + c * radii[1]]]
        }
        other => other.points(),
    }
}

/// `shape` with handle `i` moved to `p`.
pub fn move_handle(shape: &RulerShape, i: usize, p: [f64; 2]) -> RulerShape {
    let mut s = shape.clone();
    match &mut s {
        RulerShape::Straight { a, b } | RulerShape::Parallel { a, b } | RulerShape::Fisheye { a, b } => {
            if i == 0 {
                *a = p;
            } else {
                *b = p;
            }
        }
        RulerShape::Curve { points } => {
            if let Some(q) = points.get_mut(i) {
                *q = p;
            }
        }
        RulerShape::Radial { center } => *center = p,
        RulerShape::Concentric { center, radii, angle } => {
            let d = [p[0] - center[0], p[1] - center[1]];
            match i {
                0 => *center = p,
                // The first axis' handle turns the ellipse and sets that radius.
                1 => {
                    radii[0] = d[0].hypot(d[1]).max(1.0);
                    *angle = d[1].atan2(d[0]);
                }
                _ => {
                    let (s, c) = angle.sin_cos();
                    radii[1] = (-s * d[0] + c * d[1]).abs().max(1.0);
                }
            }
        }
    }
    s
}

/// The ruler a drag through `pts` draws, for kind `kind` (see [`KINDS`]); None when the drag is
/// too short for that kind (`min` document pixels).
pub fn shape_from_drag(kind: usize, pts: &[[f64; 2]], min: f64) -> Option<RulerShape> {
    let (a, b) = (*pts.first()?, *pts.last()?);
    let long = (b[0] - a[0]).hypot(b[1] - a[1]) >= min;
    match kind {
        1 => long.then_some(RulerShape::Straight { a, b }),
        2 => {
            // Evenly spaced points along the drag, at most CURVE_POINTS of them.
            let step = (pts.len() as f64 / CURVE_POINTS as f64).max(1.0);
            let mut keep: Vec<[f64; 2]> = (0..).map(|k| (k as f64 * step) as usize).take_while(|i| *i < pts.len()).map(|i| pts[i]).collect();
            if keep.last() != Some(&b) {
                keep.push(b);
            }
            keep.dedup_by(|p, q| (p[0] - q[0]).hypot(p[1] - q[1]) < min * 0.25);
            ((keep.len() >= 2 && long) || keep.len() >= 3).then_some(RulerShape::Curve { points: keep })
        }
        3 => {
            let (dx, dy) = ((b[0] - a[0]).abs(), (b[1] - a[1]).abs());
            if !long {
                return None;
            }
            // A drag mostly along one axis: a circle; otherwise the ellipse in the drag's box.
            let radii = if dx.min(dy) < min { [dx.hypot(dy); 2] } else { [dx, dy] };
            Some(RulerShape::Concentric { center: a, radii, angle: 0.0 })
        }
        4 => long.then_some(RulerShape::Parallel { a, b }),
        5 => Some(RulerShape::Radial { center: a }),
        6 => long.then_some(RulerShape::Fisheye { a, b }),
        _ => None,
    }
}

/// The rulers shown now on the active document's active layer (with their indices), with any
/// handle drag applied; none while they are hidden.
fn shown(app: &PhotocraftApp) -> Vec<(usize, RulerShape)> {
    let Some(st) = app.session.active().filter(|st| st.rulers_view.visible) else { return Vec::new() };
    st.rulers_here()
        .into_iter()
        .map(|(i, r)| match &app.special_ruler.moving {
            Some((doc, k, _, s)) if *doc == st.doc.id && *k == i => (i, s.clone()),
            _ => (i, r.shape.clone()),
        })
        .collect()
}

fn report(app: &mut PhotocraftApp, r: Result<serde_json::Value, String>) -> Option<serde_json::Value> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            app.ui.status = e;
            app.ui.status_error = true;
            None
        }
    }
}

/// With the Ruler tool, a press on a ruler's handle reshapes it, and with a Special Ruler kind
/// chosen any other press draws a new ruler. Returns true while the gesture takes the pointer
/// (so the press doesn't measure).
pub fn pointer(app: &mut PhotocraftApp, ev: ToolEvent) -> bool {
    let st = &app.special_ruler;
    if app.active_tool() != crate::state::Tool::Ruler && st.making.is_none() && st.moving.is_none() {
        return false;
    }
    let Some(id) = app.session.active().map(|st| st.doc.id) else {
        app.special_ruler = SpecialRulerState::default();
        return false;
    };
    let zoom = f64::from(ViewXform::active(app).map_or(1.0, |xf| xf.zoom).max(1e-3));
    match ev {
        ToolEvent::Down { x, y, .. } => {
            app.special_ruler.making = None;
            app.special_ruler.moving = None;
            let tol = f64::from(GRAB) / zoom;
            let hit = shown(app).into_iter().find_map(|(i, s)| handles(&s).iter().position(|h| (h[0] - x).hypot(h[1] - y) <= tol).map(|k| (i, k, s.clone())));
            if let Some((i, k, s)) = hit {
                app.special_ruler.moving = Some((id, i, k, s));
                app.special_ruler.selected = Some((id, i));
                return true;
            }
            let kind = app.ui.tool_options.special_ruler;
            if kind == 0 {
                return false;
            }
            app.special_ruler.making = Some((id, kind, vec![[x, y]]));
            true
        }
        ToolEvent::Move { x, y, .. } => {
            let st = &mut app.special_ruler;
            if let Some((doc, i, k, s)) = &mut st.moving {
                if *doc != id {
                    st.moving = None;
                    return true;
                }
                if let Some(r) = app.session.active().and_then(|d| d.doc.pira.rulers.get(*i)) {
                    *s = move_handle(&r.shape, *k, [x, y]);
                }
                return true;
            }
            if let Some((doc, _, pts)) = &mut st.making {
                if *doc != id {
                    st.making = None;
                    return true;
                }
                if pts.last().is_none_or(|p| (p[0] - x).hypot(p[1] - y) * zoom >= 4.0) {
                    pts.push([x, y]);
                }
                return true;
            }
            false
        }
        ToolEvent::Up { x, y } => {
            if let Some((doc, i, k, _)) = app.special_ruler.moving.take() {
                let shape = app.session.active().and_then(|d| d.doc.pira.rulers.get(i)).map(|r| move_handle(&r.shape, k, [x, y]));
                if doc == id
                    && let Some(shape) = shape
                {
                    let r = app.run("ruler.set", json!({"index": i, "shape": shape}));
                    report(app, r);
                }
                return true;
            }
            let Some((doc, kind, mut pts)) = app.special_ruler.making.take() else { return false };
            if doc != id {
                return true;
            }
            pts.push([x, y]);
            match shape_from_drag(kind, &pts, 8.0 / zoom) {
                Some(shape) => {
                    let mut p = serde_json::to_value(&shape).unwrap_or_default();
                    p["allLayers"] = json!(app.ui.tool_options.ruler_all_layers);
                    let r = app.run("ruler.add", p);
                    if let Some(v) = report(app, r) {
                        app.special_ruler.selected = v["index"].as_u64().map(|i| (id, i as usize));
                    }
                }
                None => {
                    app.ui.status = tl!("Drag across the canvas to draw the ruler").into();
                    app.ui.status_error = false;
                }
            }
            true
        }
    }
}

/// The Ruler tool's Special Ruler option: the kind a drag draws, whether new rulers show on
/// every layer, and (with rulers) Show, Snap, Delete Ruler and Clear Rulers.
pub fn ruler_tool_options(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let Some(st) = app.session.active() else { return };
    let (id, any, here, view) = (st.doc.id, !st.doc.pira.rulers.is_empty(), st.rulers_here().len(), st.rulers_view);
    crate::widgets::vline(ui, 22.0);
    let dim = crate::theme::Tokens::get(ui.ctx()).text_dim;
    ui.add(egui::Label::new(egui::RichText::new(tl!("Special Ruler")).color(dim)).selectable(false));
    let mut kind = app.ui.tool_options.special_ruler;
    if crate::widgets::dropdown(ui, "special-ruler", tl!("Special Ruler Kind"), &mut kind, &KINDS, 140.0) {
        app.ui.tool_options.special_ruler = kind;
    }
    crate::widgets::checkbox(ui, &mut app.ui.tool_options.ruler_all_layers, tl!("All Layers"))
        .on_hover_text(tl!("New rulers show on every layer, not just the active one"));
    if !any {
        return;
    }
    let mut run: Option<(&str, serde_json::Value)> = None;
    let (mut show, mut snap) = (view.visible, view.snap);
    if crate::widgets::checkbox(ui, &mut show, tl!("Show Rulers")).changed() {
        run = Some(("ruler.show", json!({})));
    }
    if crate::widgets::checkbox(ui, &mut snap, tl!("Snap to Rulers")).changed() {
        run = Some(("ruler.snap", json!({})));
    }
    let selected = app.special_ruler.selected.filter(|(doc, i)| *doc == id && app.session.active().is_some_and(|st| *i < st.doc.pira.rulers.len()));
    if ui.add_enabled_ui(selected.is_some(), |ui| crate::widgets::secondary_button(ui, tl!("Delete Ruler"), 0.0)).inner.clicked()
        && let Some((_, i)) = selected
    {
        run = Some(("ruler.delete", json!({"index": i})));
        app.special_ruler.selected = None;
    }
    if ui.add_enabled_ui(here > 0, |ui| crate::widgets::secondary_button(ui, tl!("Clear Rulers"), 0.0)).inner.clicked() {
        run = Some(("ruler.clear", json!({})));
        app.special_ruler.selected = None;
    }
    if let Some((cmd, p)) = run {
        let r = app.run(cmd, p);
        report(app, r);
    }
}

/// The painting tools' switch for snapping to special rulers, shown while the document has any.
pub fn snap_button(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let Some(snap) = app.session.active().filter(|st| !st.doc.pira.rulers.is_empty()).map(|st| st.rulers_view.snap) else { return };
    let tip = if snap { tl!("Snapping to special rulers (click to draw freely)") } else { tl!("Drawing freely (click to snap to special rulers)") };
    let resp = crate::icons::button_with_icon_size(ui, "ruler", 24.0, 14.0, snap, tip);
    let resp = crate::brush_picker::named(resp, "Snap to special rulers");
    if resp.clicked() {
        let r = app.run("ruler.snap", json!({}));
        report(app, r);
    }
}

/// Points around an ellipse, for drawing it.
fn ellipse_points(center: [f64; 2], radii: [f64; 2], angle: f64, n: usize) -> Vec<[f64; 2]> {
    let (s, c) = angle.sin_cos();
    (0..=n)
        .map(|k| {
            let t = std::f64::consts::TAU * k as f64 / n as f64;
            let (x, y) = (radii[0] * t.cos(), radii[1] * t.sin());
            [center[0] + c * x - s * y, center[1] + s * x + c * y]
        })
        .collect()
}

/// The lines that show what a ruler does, in document pixels, for a `w` × `h` canvas: the main
/// line first (drawn bolder), then the family of lines strokes can follow.
pub fn lines(shape: &RulerShape, w: f64, h: f64) -> Vec<Vec<[f64; 2]>> {
    let far = (w + h) * 2.0;
    let line = |o: [f64; 2], d: [f64; 2]| vec![[o[0] - d[0] * far, o[1] - d[1] * far], [o[0] + d[0] * far, o[1] + d[1] * far]];
    let unit = |v: [f64; 2]| {
        let l = v[0].hypot(v[1]);
        (l > 1e-9).then(|| [v[0] / l, v[1] / l])
    };
    let step = (w + h) / 16.0;
    match shape {
        RulerShape::Straight { a, b } => unit([b[0] - a[0], b[1] - a[1]]).map(|d| vec![line(*a, d)]).unwrap_or_default(),
        RulerShape::Curve { points } => vec![curve_path(points)],
        RulerShape::Parallel { a, b } => {
            let Some(d) = unit([b[0] - a[0], b[1] - a[1]]) else { return Vec::new() };
            let n = [-d[1], d[0]];
            let mut out = vec![vec![*a, *b]];
            let reach = (w.hypot(h) / step).ceil() as i32 + 1;
            // Lines through the canvas centre's neighbourhood, a step apart.
            let c = [w / 2.0, h / 2.0];
            let off = (c[0] - a[0]) * n[0] + (c[1] - a[1]) * n[1];
            for k in -reach..=reach {
                let t = (off / step).round() * step + f64::from(k) * step;
                out.push(line([a[0] + n[0] * t, a[1] + n[1] * t], d));
            }
            out
        }
        RulerShape::Radial { center } => {
            let mut out = vec![vec![*center, *center]];
            for k in 0..32 {
                let t = std::f64::consts::TAU * f64::from(k) / 32.0;
                out.push(vec![*center, [center[0] + t.cos() * far, center[1] + t.sin() * far]]);
            }
            out
        }
        RulerShape::Concentric { center, radii, angle } => {
            let mut out = vec![ellipse_points(*center, *radii, *angle, 96)];
            let biggest = w.hypot(h) * 2.0 / radii[0].min(radii[1]).max(1.0);
            let mut k = 0.25;
            while k <= biggest.min(40.0) {
                if (k - 1.0f64).abs() > 1e-9 {
                    out.push(ellipse_points(*center, [radii[0] * k, radii[1] * k], *angle, 96));
                }
                k += if k < 1.0 { 0.25 } else { 0.5 };
            }
            out
        }
        RulerShape::Fisheye { a, b } => {
            let Some(d) = unit([b[0] - a[0], b[1] - a[1]]) else { return Vec::new() };
            let mut out = vec![line(*a, d)];
            let n = [-d[1], d[0]];
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            let half = (b[0] - a[0]).hypot(b[1] - a[1]) / 2.0;
            for k in [-0.9, -0.6, -0.4, -0.25, -0.12, 0.12, 0.25, 0.4, 0.6, 0.9] {
                let s = [mid[0] + n[0] * half * k, mid[1] + n[1] * half * k];
                if let Some(Guide::Circle { center, r }) = guide(shape, s, 0.0) {
                    out.push(ellipse_points(center, [r, r], 0.0, 160));
                }
            }
            out
        }
    }
}

/// The special rulers' lines and handles over the canvas, and the one being drawn.
pub fn draw(app: &PhotocraftApp, painter: &egui::Painter, xf: &ViewXform) {
    let Some(st) = app.session.active() else { return };
    let (w, h) = (f64::from(st.doc.size.width), f64::from(st.doc.size.height));
    let mut rulers = shown(app);
    let preview = app.special_ruler.making.as_ref().filter(|(doc, ..)| *doc == st.doc.id).and_then(|(_, k, pts)| shape_from_drag(*k, pts, 1e-3));
    if rulers.is_empty() && preview.is_none() {
        return;
    }
    let painter = painter.with_clip_rect(painter.clip_rect().intersect(xf.rect));
    let screen = |p: [f64; 2]| xf.to_screen(p[0] as f32, p[1] as f32);
    let ok = |p: &Pos2| p.x.is_finite() && p.y.is_finite();
    let snap = st.rulers_view.snap;
    let purple = Color32::from_rgb(190, 110, 255);
    let selected = app.special_ruler.selected.filter(|(doc, _)| *doc == st.doc.id).map(|(_, i)| i);
    if let Some(p) = preview {
        rulers.push((usize::MAX, p));
    }
    for (i, shape) in &rulers {
        let strong = *i == usize::MAX || Some(*i) == selected;
        let family = Stroke::new(1.0, purple.gamma_multiply(if snap { 0.35 } else { 0.2 }));
        let main = Stroke::new(if strong { 2.0 } else { 1.5 }, purple.gamma_multiply(if snap { 1.0 } else { 0.55 }));
        for (k, l) in lines(shape, w, h).into_iter().enumerate() {
            let pts: Vec<Pos2> = l.into_iter().map(screen).filter(ok).collect();
            if pts.len() >= 2 {
                if k == 0 {
                    painter.add(egui::Shape::line(pts.clone(), Stroke::new(3.0, Color32::from_black_alpha(70))));
                }
                painter.add(egui::Shape::line(pts, if k == 0 { main } else { family }));
            }
        }
        for hd in handles(shape) {
            let p = screen(hd);
            if ok(&p) {
                painter.circle(p, 6.0, Color32::from_black_alpha(90), Stroke::new(2.0, purple));
                painter.circle_filled(p, 2.0, purple);
            }
        }
    }
}

#[cfg(test)]
mod tests;
