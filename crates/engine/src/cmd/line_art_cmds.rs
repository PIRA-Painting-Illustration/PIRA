//! Colouring line art, Clip Studio-style: reference layers (Layer › Reference Layer), what the
//! Paint Bucket and Magic Wand look at to find an area (`sample`: the layer, all layers, or only
//! the reference layers) with Close Gap and Expand (`closeGap`, `expand`), and Lasso Fill
//! (`paint.lassoFill`: a Paint Bucket drag).

use photocraft_algo::selection::{GapOptions, MAX_EXPAND, MAX_GAP};
use photocraft_doc::{Document, Layer, LayerId};
use photocraft_geom::Rect;
use serde_json::{Value, json};

use crate::commands::CommandSpec;
use crate::{EngineError, Result, Session};

const SET_REFERENCE: &str = "layer.setReference";
const SET_DRAFT: &str = "layer.setDraft";
const SET_LINE_COLOR: &str = "layer.setLineColor";
const SET_BORDER: &str = "layer.setBorder";
/// The border a toggle turns on: 4 px of white, a sticker outline.
pub const DEFAULT_BORDER: photocraft_doc::Border = photocraft_doc::Border { width: 4, color: [255, 255, 255] };
/// Widest border.
pub const MAX_BORDER: u16 = 100;
/// The line colour a toggle turns on: a sketch blue.
pub const DEFAULT_LINE_COLOR: [u8; 3] = [0x3a, 0x7b, 0xd5];
const LASSO_FILL: &str = "paint.lassoFill";

/// What a fill or wand looks at to find its area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sample {
    /// The layer it works on.
    Layer,
    /// Every visible layer (Photoshop's Sample All Layers).
    All,
    /// Only the visible reference layers.
    Reference,
}

fn bad(cmd: &str, msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

/// `sample` (`layer|all|reference`), or the older `sampleAllLayers` flag when it's absent.
pub fn sample_param(cmd: &str, p: &Value) -> Result<Sample> {
    match p.get("sample") {
        None | Some(Value::Null) => Ok(if p.get("sampleAllLayers").and_then(Value::as_bool).unwrap_or(false) { Sample::All } else { Sample::Layer }),
        Some(Value::String(v)) => match v.as_str() {
            "layer" | "current" => Ok(Sample::Layer),
            "all" => Ok(Sample::All),
            "reference" => Ok(Sample::Reference),
            _ => Err(bad(cmd, format!("`sample` must be layer, all or reference, not `{v}`"))),
        },
        Some(v) => Err(bad(cmd, format!("`sample` must be layer, all or reference, not {v}"))),
    }
}

fn whole(cmd: &str, p: &Value, key: &str, max: u32) -> Result<u32> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(0),
        Some(v) => match v.as_f64() {
            Some(n) if n.is_finite() && (0.0..=f64::from(max)).contains(&n) => Ok(n.round() as u32),
            _ => Err(bad(cmd, format!("`{key}` must be 0 to {max} pixels"))),
        },
    }
}

/// `closeGap` and `expand` (pixels; 0 = off).
pub fn gap_param(cmd: &str, p: &Value) -> Result<GapOptions> {
    Ok(GapOptions { close_gap: whole(cmd, p, "closeGap", MAX_GAP)?, expand: whole(cmd, p, "expand", MAX_EXPAND)? })
}

/// The document with everything but its reference layers hidden, or `None` when no visible
/// layer is a reference layer. A reference group counts with all of its contents.
pub fn reference_only(doc: &Document) -> Option<Document> {
    fn keep(layers: &mut [Layer]) -> bool {
        let mut any = false;
        for l in layers {
            if l.pira.reference {
                any |= l.visible;
            } else if let Some(children) = l.children_mut() {
                any |= keep(children) && l.visible;
            } else {
                l.visible = false;
            }
        }
        any
    }
    let mut d = doc.clone();
    keep(&mut d.layers).then_some(d)
}

/// The document with its draft layers hidden, or `None` when it has none showing.
pub fn without_drafts(doc: &Document) -> Option<Document> {
    fn hide(layers: &mut [Layer]) -> bool {
        let mut any = false;
        for l in layers {
            if l.pira.draft && l.visible {
                l.visible = false;
                any = true;
            } else if let Some(children) = l.children_mut() {
                any |= hide(children);
            }
        }
        any
    }
    let mut d = doc.clone();
    hide(&mut d.layers).then_some(d)
}

/// 8-bit RGBA over the canvas for finding an area: `layer`'s pixels, the visible composite, or
/// the reference layers' composite.
pub fn sample_image(s: &Session, cmd: &str, layer: Option<LayerId>, sample: Sample) -> Result<(Rect, Vec<[u8; 4]>)> {
    let d = s.active().ok_or(EngineError::NoDocument)?;
    let area = d.doc.bounds();
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let composite = |doc: &Document| photocraft_compose::render(doc, area).px.iter().map(|p| p.map(q)).collect();
    match sample {
        Sample::Layer => match layer.or(d.active_layer).and_then(|id| d.doc.layer(id)).and_then(Layer::surface) {
            Some(surf) => Ok((area, photocraft_algo::selection::rgba8_image(surf, area))),
            None => Ok((area, composite(&d.doc))),
        },
        Sample::All => Ok((area, composite(without_drafts(&d.doc).as_ref().unwrap_or(&d.doc)))),
        Sample::Reference => {
            let doc = reference_only(without_drafts(&d.doc).as_ref().unwrap_or(&d.doc))
                .ok_or_else(|| bad(cmd, "no reference layer: mark your line art with Layer › Reference Layer"))?;
            Ok((area, composite(&doc)))
        }
    }
}

/// The `points` param as `(x, y)` pairs; at least three.
fn points(cmd: &str, p: &Value) -> Result<Vec<(f32, f32)>> {
    let pts: Vec<(f32, f32)> = p
        .get("points")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| {
                    let v = v.as_array()?;
                    let (x, y) = (v.first()?.as_f64()? as f32, v.get(1)?.as_f64()? as f32);
                    (x.is_finite() && y.is_finite()).then_some((x, y))
                })
                .collect()
        })
        .unwrap_or_default();
    if pts.len() < 3 {
        return Err(bad(cmd, "`points` needs at least three [x, y] points"));
    }
    Ok(pts)
}

/// Pixels a closed area can't cross in Enclosed mode: dark, mostly opaque pixels (the ink).
fn ink(px: [u8; 4]) -> bool {
    let lum = (u32::from(px[0]) * 30 + u32::from(px[1]) * 59 + u32::from(px[2]) * 11) / 100;
    px[3] >= 128 && lum < 128
}

/// Enclosed: the areas between the lines (`img` over `area`) that lie wholly inside the lasso
/// (`cov` over `window`, 0..1).
fn enclosed(img: &[[u8; 4]], area: Rect, cov: &[f32], window: Rect) -> Vec<u8> {
    let (w, h) = (window.width() as usize, window.height() as usize);
    let stride = area.width() as usize;
    let (ox, oy) = ((window.x0 - area.x0) as usize, (window.y0 - area.y0) as usize);
    let at = |i: usize| img[(oy + i / w) * stride + ox + i % w];
    let inside = |i: usize| cov[i] >= 0.5;
    let mut seen = vec![false; w * h];
    let mut out = vec![0u8; w * h];
    let mut component = Vec::new();
    for start in 0..w * h {
        if seen[start] || !inside(start) || ink(at(start)) {
            continue;
        }
        // One area between the lines, 4-connected; it counts only if it never leaves the lasso.
        component.clear();
        let mut whole = true;
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            component.push(i);
            // The window has a margin outside the lasso (except where the canvas cuts it), so an
            // area reaching past the window has already left the lasso.
            let (x, y) = (i % w, i / w);
            if !inside(i) {
                whole = false;
            }
            let mut neighbours = [None; 4];
            if x > 0 {
                neighbours[0] = Some(i - 1);
            }
            if x + 1 < w {
                neighbours[1] = Some(i + 1);
            }
            if y > 0 {
                neighbours[2] = Some(i - w);
            }
            if y + 1 < h {
                neighbours[3] = Some(i + w);
            }
            for j in neighbours.into_iter().flatten() {
                if !seen[j] && !ink(at(j)) {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        if whole {
            for &i in &component {
                out[i] = 255;
            }
        }
    }
    out
}

/// Paint Bucket drag: fill inside a lasso. `kind`: `shape` (the lasso itself), `enclosed` (the
/// areas between the lines wholly inside it, found like the bucket's `sample`; `layer` means all
/// layers here, since the layer filled is rarely the one with the lines) or `unfilled` (only the
/// target's empty pixels inside it, for the specks a fill missed).
fn lasso_fill(s: &mut Session, p: &Value) -> Result<Value> {
    let pts = points(LASSO_FILL, p)?;
    let kind = p.get("kind").and_then(Value::as_str).unwrap_or("shape");
    if !matches!(kind, "shape" | "enclosed" | "unfilled") {
        return Err(bad(LASSO_FILL, format!("`kind` must be shape, enclosed or unfilled, not `{kind}`")));
    }
    let aa = p.get("antiAlias").and_then(Value::as_bool).unwrap_or(true);
    let opacity = crate::paint_cmds::opacity(p, LASSO_FILL)?;
    let mode = crate::paint_cmds::fill_mode(p)?;
    let gap = gap_param(LASSO_FILL, p)?;
    let c = crate::paint_cmds::color(p.get("color"), s.tools.foreground);
    let inks = s.tools.picked_cmyk([c[0], c[1], c[2]]).filter(|_| c[3] >= 1.0);
    let layer = if p.get("layer").is_some() { Some(crate::commands::layer_param(s, p)?) } else { s.active().and_then(|st| st.active_layer) };
    let area = s.active().ok_or(EngineError::NoDocument)?.doc.bounds();
    let (mut lo, mut hi) = ((f32::INFINITY, f32::INFINITY), (f32::NEG_INFINITY, f32::NEG_INFINITY));
    for &(x, y) in &pts {
        lo = (lo.0.min(x), lo.1.min(y));
        hi = (hi.0.max(x), hi.1.max(y));
    }
    let margin = 1 + gap.expand as i32;
    let window =
        Rect::new(lo.0.floor() as i32 - margin, lo.1.floor() as i32 - margin, hi.0.ceil() as i32 + margin, hi.1.ceil() as i32 + margin).intersect(&area);
    if window.is_empty() {
        return Ok(json!({"filled": false}));
    }
    let cov = photocraft_algo::selection::polygon_window(&pts, area, window, aa && kind == "shape");
    let mut mask: Vec<u8> = if kind == "enclosed" {
        let sample = match sample_param(LASSO_FILL, p)? {
            Sample::Layer => Sample::All,
            other => other,
        };
        let (img_area, img) = sample_image(s, LASSO_FILL, layer, sample)?;
        enclosed(&img, img_area, &cov, window)
    } else {
        cov.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
    };
    if gap.expand > 0 && kind != "shape" {
        photocraft_algo::selection::expand_marks(&mut mask, window.width() as usize, window.height() as usize, gap.expand);
    }
    let unfilled = kind == "unfilled";
    let filled = s.edit("Lasso Fill", |doc, _| {
        let sel = doc.selection.clone();
        let (surf, lock) = crate::channel_cmds::target_surface(doc, layer, p)?;
        let mut mask = std::mem::take(&mut mask);
        if unfilled {
            let w = window.width() as usize;
            for (i, m) in mask.iter_mut().enumerate() {
                let (x, y) = (window.x0 + (i % w) as i32, window.y0 + (i / w) as i32);
                if *m != 0 && surf.pixel(x, y).last().copied().unwrap_or(0.0) >= 0.5 {
                    *m = 0;
                }
            }
        }
        if mask.iter().all(|&m| m == 0) {
            return Ok(false);
        }
        let region = photocraft_algo::selection::Region { bbox: window, mask };
        photocraft_algo::paint::bucket_fill_region_inks(surf, &region, opacity, sel.as_ref(), lock, |_, _| c, inks, mode);
        surf.prune();
        Ok(true)
    })?;
    Ok(json!({"filled": filled}))
}

fn enabled(s: &Session) -> std::result::Result<(), String> {
    let st = s.active().ok_or("no document open")?;
    if st.selected_layers().is_empty() { Err("no layer selected".into()) } else { Ok(()) }
}

/// The layer named by `layer`, or the selected layers.
fn target_layers(s: &Session, cmd: &str, p: &Value) -> Result<Vec<LayerId>> {
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let ids = match crate::commands::explicit_layer(s, p)? {
        Some(id) => vec![id],
        None => st.selected_layers(),
    };
    if ids.is_empty() {
        return Err(bad(cmd, "no layer selected"));
    }
    for id in &ids {
        st.doc.layer(*id).ok_or(EngineError::NoLayer(*id))?;
    }
    Ok(ids)
}

/// Turn a layer flag on or off (`key`, or a toggle going by the first layer), as one undo step.
fn set_flag(s: &mut Session, p: &Value, cmd: &str, key: &str, steps: (&str, &str), flag: fn(&mut Layer) -> &mut bool) -> Result<Value> {
    let ids = target_layers(s, cmd, p)?;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let mut first = st.doc.layer(ids[0]).ok_or(EngineError::NoLayer(ids[0]))?.clone();
    let on = match p.get(key) {
        None | Some(Value::Null) => !*flag(&mut first),
        Some(Value::Bool(v)) => *v,
        Some(v) => return Err(bad(cmd, format!("`{key}` must be true or false, not {v}"))),
    };
    let changed = ids.iter().any(|id| st.doc.layer(*id).is_some_and(|l| *flag(&mut l.clone()) != on));
    if changed {
        s.edit(if on { steps.0 } else { steps.1 }, |doc, _| {
            for id in &ids {
                *flag(doc.layer_mut(*id).ok_or(EngineError::NoLayer(*id))?) = on;
            }
            Ok(())
        })?;
        if let Some(st) = s.active_mut() {
            // A flag on the layer: nothing on the canvas changes.
            st.last_damage = Some(Rect::EMPTY);
        }
    }
    Ok(json!({key: on, "layers": ids.iter().map(|id| id.0).collect::<Vec<_>>()}))
}

/// Layer › Reference Layer: mark (or unmark) the layer, or the selected layers.
fn set_reference(s: &mut Session, p: &Value) -> Result<Value> {
    set_flag(s, p, SET_REFERENCE, "reference", ("Reference Layer", "Not a Reference Layer"), |l| &mut l.pira.reference)
}

/// Layer › Draft Layer: mark (or unmark) the layer, or the selected layers.
fn set_draft(s: &mut Session, p: &Value) -> Result<Value> {
    set_flag(s, p, SET_DRAFT, "draft", ("Draft Layer", "Not a Draft Layer"), |l| &mut l.pira.draft)
}

/// The Color Overlay a line colour stands for.
fn line_overlay(rgb: [u8; 3]) -> photocraft_doc::Effect {
    let c = |v: u8| f32::from(v) / 255.0;
    photocraft_doc::Effect::ColorOverlay {
        common: photocraft_doc::FxCommon::new(photocraft_doc::BlendMode::Normal, 1.0),
        color: photocraft_doc::Color::rgba(c(rgb[0]), c(rgb[1]), c(rgb[2]), 1.0),
    }
}

/// Set a layer's line colour, keeping its Color Overlay in step.
fn apply_line_color(l: &mut Layer, color: Option<[u8; 3]>) {
    if let Some(old) = l.pira.line_color {
        let ours = line_overlay(old);
        l.effects.items.retain(|e| *e != ours);
    }
    if let Some(rgb) = color {
        l.effects.items.push(line_overlay(rgb));
        l.effects.enabled = true;
    }
    l.effects.psd_raw = None;
    l.pira.line_color = color;
}

/// `#rrggbb` as bytes.
fn hex_color(cmd: &str, v: &str) -> Result<[u8; 3]> {
    let h = v.trim_start_matches('#');
    let byte = |i: usize| h.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok());
    match (h.len(), byte(0), byte(2), byte(4)) {
        (6, Some(r), Some(g), Some(b)) => Ok([r, g, b]),
        _ => Err(bad(cmd, format!("`color` must be #rrggbb, not `{v}`"))),
    }
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// The outside Stroke a border stands for.
fn border_stroke(b: photocraft_doc::Border) -> photocraft_doc::Effect {
    let c = |v: u8| f32::from(v) / 255.0;
    photocraft_doc::Effect::Stroke(photocraft_doc::StrokeFx {
        common: photocraft_doc::FxCommon::new(photocraft_doc::BlendMode::Normal, 1.0),
        size: f32::from(b.width),
        position: photocraft_doc::StrokePosition::Outside,
        paint: photocraft_doc::FxPaint::Color(photocraft_doc::Color::rgba(c(b.color[0]), c(b.color[1]), c(b.color[2]), 1.0)),
        overprint: false,
    })
}

/// Set a layer's border, keeping its Stroke effect in step.
fn apply_border(l: &mut Layer, border: Option<photocraft_doc::Border>) {
    if let Some(old) = l.pira.border {
        let ours = border_stroke(old);
        l.effects.items.retain(|e| *e != ours);
    }
    if let Some(b) = border {
        l.effects.items.push(border_stroke(b));
        l.effects.enabled = true;
    }
    l.effects.psd_raw = None;
    l.pira.border = border;
}

/// Layer › Border Effect: an outline around the layer's pixels (`width` 1..100 px or `null` for
/// off, `color` "#rrggbb"; with neither, it toggles 4 px of white). A width alone keeps the colour.
fn set_border(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = target_layers(s, SET_BORDER, p)?;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let current = st.doc.layer(ids[0]).ok_or(EngineError::NoLayer(ids[0]))?.pira.border;
    let color = match p.get("color") {
        None | Some(Value::Null) => None,
        Some(Value::String(v)) => Some(hex_color(SET_BORDER, v)?),
        Some(v) => return Err(bad(SET_BORDER, format!("`color` must be #rrggbb, not {v}"))),
    };
    let border = match p.get("width") {
        Some(Value::Null) => None,
        None if color.is_none() => {
            if current.is_some() {
                None
            } else {
                Some(DEFAULT_BORDER)
            }
        }
        None => Some(photocraft_doc::Border { color: color.unwrap_or(DEFAULT_BORDER.color), ..current.unwrap_or(DEFAULT_BORDER) }),
        Some(v) => match v.as_f64() {
            Some(w) if w.is_finite() && (1.0..=f64::from(MAX_BORDER)).contains(&w.round()) => {
                let base = current.unwrap_or(DEFAULT_BORDER);
                Some(photocraft_doc::Border { width: w.round() as u16, color: color.unwrap_or(base.color) })
            }
            _ => return Err(bad(SET_BORDER, format!("`width` must be 1 to {MAX_BORDER} pixels, or null"))),
        },
    };
    let changed = ids.iter().any(|id| st.doc.layer(*id).is_some_and(|l| l.pira.border != border));
    if changed {
        s.edit(if border.is_some() { "Border Effect" } else { "No Border Effect" }, |doc, _| {
            for id in &ids {
                apply_border(doc.layer_mut(*id).ok_or(EngineError::NoLayer(*id))?, border);
            }
            Ok(())
        })?;
    }
    Ok(json!({"width": border.map(|b| b.width), "color": border.map(|b| hex(b.color)), "layers": ids.iter().map(|id| id.0).collect::<Vec<_>>()}))
}

/// Layer › Line Color: show the layer's pixels in one colour (`color` "#rrggbb", `null` for off;
/// omitted, it toggles a sketch blue).
fn set_line_color(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = target_layers(s, SET_LINE_COLOR, p)?;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let current = st.doc.layer(ids[0]).ok_or(EngineError::NoLayer(ids[0]))?.pira.line_color;
    let color = match p.get("color") {
        None => {
            if current.is_some() {
                None
            } else {
                Some(DEFAULT_LINE_COLOR)
            }
        }
        Some(Value::Null) => None,
        Some(Value::String(h)) => {
            let h = h.trim_start_matches('#');
            let byte = |i: usize| h.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok());
            match (h.len(), byte(0), byte(2), byte(4)) {
                (6, Some(r), Some(g), Some(b)) => Some([r, g, b]),
                _ => return Err(bad(SET_LINE_COLOR, format!("`color` must be #rrggbb or null, not `{h}`"))),
            }
        }
        Some(v) => return Err(bad(SET_LINE_COLOR, format!("`color` must be #rrggbb or null, not {v}"))),
    };
    let changed = ids.iter().any(|id| st.doc.layer(*id).is_some_and(|l| l.pira.line_color != color));
    if changed {
        s.edit(if color.is_some() { "Line Color" } else { "No Line Color" }, |doc, _| {
            for id in &ids {
                apply_line_color(doc.layer_mut(*id).ok_or(EngineError::NoLayer(*id))?, color);
            }
            Ok(())
        })?;
    }
    let hex = color.map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]));
    Ok(json!({"color": hex, "layers": ids.iter().map(|id| id.0).collect::<Vec<_>>()}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: SET_REFERENCE,
            label: "Reference Layer",
            menu: &["Layer"],
            shortcut: None,
            params: r#"{"reference":bool? (omitted: toggle),"layer":id|name? (no layer: the selected layers)} (the Paint Bucket and Magic Wand with sample=reference find their area from these layers)"#,
            enabled,
            run: set_reference,
            journal: true,
        },
        CommandSpec {
            id: SET_DRAFT,
            label: "Draft Layer",
            menu: &["Layer"],
            shortcut: None,
            params: r#"{"draft":bool? (omitted: toggle),"layer":id|name? (no layer: the selected layers)} (draft layers show while you work but are left out of exports, prints and what fills look at)"#,
            enabled,
            run: set_draft,
            journal: true,
        },
        CommandSpec {
            id: SET_BORDER,
            label: "Border Effect",
            menu: &["Layer"],
            shortcut: None,
            params: r##"{"width":1..100 px|null? (null: off),"color":"#rrggbb"? (with neither: toggle 4 px of white),"layer":id|name? (no layer: the selected layers)} (an outside Stroke effect)"##,
            enabled,
            run: set_border,
            journal: true,
        },
        CommandSpec {
            id: SET_LINE_COLOR,
            label: "Line Color",
            menu: &["Layer"],
            shortcut: None,
            params: r##"{"color":"#rrggbb"|null? (omitted: toggle a sketch blue),"layer":id|name? (no layer: the selected layers)} (the layer's pixels show in this colour, alpha kept; a Color Overlay effect)"##,
            enabled,
            run: set_line_color,
            journal: true,
        },
        CommandSpec {
            id: LASSO_FILL,
            label: "Lasso Fill",
            menu: &[],
            shortcut: None,
            params: r##"{"points":[[x,y],…] (3 or more),"kind":"shape|enclosed|unfilled"="shape" (enclosed: the areas between the lines wholly inside the lasso; unfilled: only the empty pixels inside it),"sample":"layer|all|reference"? (enclosed: where the lines are; layer means all),"expand":0..32 px=0,"antiAlias":bool=true,"color":"#rrggbb"=foreground,"opacity":1..100=100,"mode":"normal|multiply|…|clear"="normal","target":"pixels"|"mask"|…=Channels panel target}"##,
            enabled: crate::commands::has_paintable,
            run: lasso_fill,
            journal: true,
        },
    ]
}

#[cfg(test)]
mod tests;
