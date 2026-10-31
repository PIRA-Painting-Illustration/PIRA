//! PIRA's app menu: the touch themes' replacement for the desktop menu row. One button (the P
//! mark) opens a sheet of large tiles, one per top-level menu, each naming what it holds; a tile
//! opens that menu's items beside it, with the same rows, checks, shortcuts and commands as the
//! menu bar, so nothing becomes unreachable.

use super::invoke::invoke;
use super::items::{MenuItem, menu_items};
use super::render::render_level;
use super::*;

/// The tiles, in sheet order: the menu (as in [`TOP_MENUS`]) and a hint of what is in it.
const TILES: [(&str, &str); 10] = [
    ("File", "New, open, save, export"),
    ("Edit", "Undo, transform, preferences"),
    ("Image", "Canvas size, rotate, adjustments"),
    ("Layer", "New, merge, masks, styles"),
    ("Select", "All, invert, modify"),
    ("Filter", "Blur, sharpen, liquify"),
    ("Type", "Fonts and text layers"),
    ("View", "Zoom, rulers, guides"),
    ("Window", "Panels and workspaces"),
    ("Help", "Search, about"),
];

/// Width and height of a tile (points).
const TILE: egui::Vec2 = egui::vec2(200.0, 52.0);

/// A tile's two lines: the menu's name, and its hint in the faint colour.
fn tile_text(ui: &egui::Ui, title: &str, hint: &str) -> egui::text::LayoutJob {
    let t = crate::theme::Tokens::get(ui.ctx());
    let mut job = egui::text::LayoutJob::default();
    job.append(title, 0.0, egui::TextFormat { font_id: crate::theme::semibold(14.5), color: t.text, ..Default::default() });
    job.append("\n", 0.0, egui::TextFormat::default());
    job.append(hint, 0.0, egui::TextFormat { font_id: egui::FontId::proportional(11.5), color: t.text_faint, ..Default::default() });
    job
}

/// The P button and its sheet. Runs the chosen command like the menu bar does.
pub fn app_menu(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let lang = crate::i18n::current();
    let mut clicked: Option<String> = None;
    let mut nav = crate::menu_nav::Nav::load(ui.ctx());
    let mut items: Option<Vec<MenuItem>> = None;
    let app_ref: &PhotocraftApp = app;
    let mark = egui::Button::new(egui::RichText::new("P").font(crate::theme::semibold(16.0)).color(t.primary_text))
        .fill(t.accent)
        .corner_radius(t.radius_sm)
        .min_size(egui::vec2(32.0, 32.0));
    egui::containers::menu::MenuButton::from_button(mark).ui(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        ui.label(egui::RichText::new("PIRA").font(crate::theme::semibold(13.0)).color(t.accent_text));
        let items = items.get_or_insert_with(|| menu_items(app_ref));
        egui::Grid::new("pira-app-menu").num_columns(2).spacing(egui::vec2(8.0, 8.0)).show(ui, |ui| {
            for (i, (top, hint)) in TILES.iter().enumerate() {
                let mine: Vec<&MenuItem> = items.iter().filter(|it| it.path.first().map(String::as_str) == Some(*top)).collect();
                let title = crate::i18n::tr(lang, top);
                let button = egui::Button::new(tile_text(ui, title, crate::i18n::tr(lang, hint)))
                    .right_text(egui::containers::menu::SubMenuButton::arrow_atom(Some(t.text_faint)))
                    .fill(t.card)
                    .stroke(egui::Stroke::new(1.0, t.card_border))
                    .corner_radius(t.radius)
                    .min_size(TILE);
                egui::containers::menu::SubMenuButton::from_button(button).ui(ui, |ui| {
                    ui.set_min_width(240.0);
                    if mine.is_empty() {
                        ui.weak(crate::i18n::tr(lang, "(coming soon)"));
                    }
                    render_level(ui, &mine, 1, &mut clicked, &mut nav);
                });
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    });
    nav.store(ui.ctx());
    if let Some(id) = clicked {
        let ctx = ui.ctx().clone();
        if let Err(e) = invoke(app, &ctx, &id, json!({})) {
            app.ui.status = e;
            app.ui.status_error = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every top-level menu has a tile, so the sheet reaches everything the menu bar did.
    #[test]
    fn every_menu_has_a_tile() {
        for top in TOP_MENUS {
            assert!(TILES.iter().any(|(t, _)| *t == top), "{top}");
        }
        assert_eq!(TILES.len(), TOP_MENUS.len());
    }
}
