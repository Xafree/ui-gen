//! The `Library` dropdown of a tab of a library project: local components, local styles (both even
//! when empty), then one row per source family with its imported components, and any enabled
//! library. Components are rows, not tiles, and act like tiles: a local one places on a click, all
//! drag onto the canvas, and the context menus are the same. Each row folds, per tab.

use std::collections::HashMap;

use egui::{Align2, CornerRadius, Id, Rect, Response, Sense, Ui, pos2, vec2};
use figcraft_doc::NodeId;
use serde_json::json;

use super::imported::{self, Block};
use super::{Act, IMPORT_WINDOW, Listing, Source, StyleRow, Tile, style_icon};
use crate::FigApp;
use crate::a11y::{self, Tip};
use crate::i18n::tr;
use crate::icons::{self, Icon};
use crate::source_family::Family;
use crate::theme::{Tokens, regular, semibold};

pub(crate) const FONT_SIZE: f32 = 12.5;
pub(crate) const NOTE_SIZE: f32 = 11.0;
pub(crate) const ROW: f32 = 26.0;
pub(crate) const TITLE: f32 = 28.0;
pub(crate) const SECTION_PAD: f32 = 10.0;
pub(crate) const RADIUS: u8 = 6;
pub(crate) const SIDE: f32 = 8.0;
pub(crate) const INNER: f32 = 14.0;
pub(crate) const ICON: f32 = 14.0;
pub(crate) const ICON_GAP: f32 = 6.0;

const LIBRARY: &str = "library";
const LOCAL: &str = "local";
const STYLES: &str = "styles";

/// The left edge of a row's contents at `depth`, from the panel's edge.
pub(crate) fn indent(depth: usize) -> f32 {
    match depth {
        0 => INNER,
        _ => INNER + 16.0 + 18.0 * (depth - 1) as f32,
    }
}

/// What the dropdown remembers: its folds per tab (by document uid) and its search.
#[derive(Default)]
pub struct LibraryPanel {
    pub folds: HashMap<u64, HashMap<String, bool>>,
    pub search: bool,
    pub filter: String,
    pub reveal: bool,
}

impl LibraryPanel {
    /// Whether row `key` of tab `uid` is open: as last left, or by default all but the local styles.
    pub fn is_open(&self, uid: u64, key: &str) -> bool {
        self.folds.get(&uid).and_then(|tab| tab.get(key)).copied().unwrap_or(key != STYLES)
    }

    fn toggle(&mut self, uid: u64, key: &str) {
        let open = self.is_open(uid, key);
        self.folds.entry(uid).or_default().insert(key.to_string(), !open);
    }
}

/// One row of the dropdown for a source: a family, or a source removed from the project whose page
/// is still in the file.
#[derive(Clone)]
pub(super) struct FamilyRow {
    pub key: String,
    pub label: String,
    pub package: String,
    pub removed: bool,
    pub tiles: Vec<(Source, Tile)>,
}

/// What a family's row says: `heroui 3.2.7`, also for a family of one package.
pub(crate) fn family_label(family: &Family) -> String {
    if family.label.ends_with(&family.version) { family.label.clone() } else { format!("{} {}", family.label, family.version) }
}

fn tiles_of(blocks: &[&Block]) -> Vec<(Source, Tile)> {
    blocks.iter().flat_map(|block| block.tiles.iter().map(|tile| (Source::Imported(block.key.clone()), tile.clone()))).collect()
}

/// The rows for `families` and the imported `blocks`. A package with no component of its own and
/// nothing imported (a stylesheet) has no row; nor has a family with no match while `searching`.
pub(super) fn family_rows(families: &[Family], blocks: &[Block], searching: bool) -> Vec<FamilyRow> {
    let mut rows = Vec::new();
    for family in families {
        let own: Vec<&Block> = blocks.iter().filter(|block| !block.removed && family.packages.contains(&block.package)).collect();
        let tiles = tiles_of(&own);
        if (tiles.is_empty() && !family.has_components) || (searching && tiles.is_empty()) {
            continue;
        }
        let package = family.packages.first().cloned().unwrap_or_default();
        rows.push(FamilyRow { key: format!("family/{}", family.name), label: family_label(family), package, removed: false, tiles });
    }
    for block in blocks.iter().filter(|block| block.removed) {
        let label = format!("{} {}", block.package, block.version);
        let tiles = tiles_of(&[block]);
        rows.push(FamilyRow { key: format!("removed/{}", block.key), label, package: block.package.clone(), removed: true, tiles });
    }
    rows
}

fn paint_chevron(ui: &Ui, x: f32, y: f32, open: bool, colour: egui::Color32) {
    let rect = Rect::from_center_size(pos2(x + ICON / 2.0, y), vec2(ICON, ICON));
    icons::paint(ui.painter(), rect, if open { Icon::Chevron } else { Icon::ChevronRight }, colour);
}

/// A 24 px icon button on a title row, painted only while `shown`; returns whether it was clicked.
pub(crate) fn title_button(ui: &mut Ui, rect: Rect, id: Id, tip: &str, icon: Icon, shown: bool, active: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let resp = ui.interact(rect, id, Sense::click()).tip(tip);
    if resp.hovered() || active {
        ui.painter().rect_filled(rect, CornerRadius::same(5), t.hover);
    }
    if shown || resp.has_focus() || active {
        icons::paint(ui.painter(), rect.shrink(5.0), icon, t.text_secondary);
    }
    a11y::ring_around(ui, rect, &resp);
    resp
}

/// A foldable row one level in (`Composants locaux`, `heroui 3.2.7`) with its count.
fn fold_row(ui: &mut Ui, w: f32, id: Id, label: &str, count: usize, note: Option<&str>, open: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(w, ROW), Sense::click());
    let resp = ui.interact(rect, id, Sense::click()).union(resp);
    a11y::toggle(&resp, label, open);
    let bg = rect.shrink2(vec2(SIDE, 0.0));
    if resp.hovered() {
        ui.painter().rect_filled(bg, CornerRadius::same(RADIUS), t.hover);
    }
    a11y::ring_around(ui, bg, &resp);
    let x = rect.min.x + indent(1);
    paint_chevron(ui, x, rect.center().y, open, t.text_secondary);
    let count =
        ui.painter().text(pos2(bg.max.x - 6.0, rect.center().y), Align2::RIGHT_CENTER, count.to_string(), regular(NOTE_SIZE), t.text_tertiary);
    let left = x + ICON + ICON_GAP;
    let note_w = note.map_or(0.0, |note| {
        let galley = ui.painter().layout_no_wrap(note.to_string(), regular(NOTE_SIZE), t.text_tertiary);
        let width = galley.size().x;
        ui.painter().galley(pos2(count.min.x - 8.0 - width, rect.center().y - galley.size().y / 2.0), galley, t.text_tertiary);
        width + 8.0
    });
    crate::bidi::paint_name(
        ui.painter(),
        pos2(left, rect.center().y),
        label,
        regular(FONT_SIZE),
        t.text_secondary,
        (count.min.x - left - note_w - 8.0).max(20.0),
    );
    resp
}

/// A secondary line at the depth of the components, wrapped to the panel.
fn empty_note(ui: &mut Ui, w: f32, text: &str) {
    let t = Tokens::get(ui.ctx());
    let left = indent(2);
    let galley = ui.painter().layout(text.to_string(), regular(FONT_SIZE), t.text_tertiary, (w - left - INNER).max(40.0));
    let height = (galley.size().y + 6.0).max(ROW);
    let (rect, _) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
    ui.painter().galley(pos2(rect.min.x + left, rect.center().y - galley.size().y / 2.0), galley, t.text_tertiary);
}

/// Whether the selection is `id` or inside it.
fn holds_selection(doc: &figcraft_doc::Document, selection: &[NodeId], id: NodeId) -> bool {
    selection.iter().any(|s| *s == id || doc.ancestors(*s).contains(&id))
}

struct List<'a> {
    w: f32,
    doc: &'a figcraft_doc::Document,
    selection: &'a [NodeId],
    wanted: Vec<(Source, NodeId)>,
    act: Option<Act>,
}

impl List<'_> {
    /// A component's row: it acts like a tile, and an imported one also opens its import page.
    fn component(&mut self, ui: &mut Ui, src: &Source, item: &Tile) {
        let t = Tokens::get(ui.ctx());
        let (rect, _) = ui.allocate_exact_size(vec2(self.w, ROW), Sense::hover());
        let resp = ui.interact(rect, Id::unique(("asset", src, item.id.0)), Sense::click_and_drag());
        let bg = rect.shrink2(vec2(SIDE, 0.0));
        let selected = holds_selection(self.doc, self.selection, item.id);
        a11y::list_item(&resp, &item.name, selected);
        if selected {
            ui.painter().rect_filled(bg, CornerRadius::same(RADIUS), t.selected_row);
        } else if resp.hovered() {
            ui.painter().rect_filled(bg, CornerRadius::same(RADIUS), t.hover);
        }
        a11y::ring_around(ui, bg, &resp);
        let x = rect.min.x + indent(2);
        let icon_rect = Rect::from_center_size(pos2(x + ICON / 2.0, rect.center().y), vec2(ICON, ICON));
        icons::paint(ui.painter(), icon_rect, if item.set { Icon::ComponentSet } else { Icon::Component }, t.component);
        let left = x + ICON + ICON_GAP;
        let own_hidden = *src == Source::Local && item.hidden;
        let colour = if own_hidden { t.text_secondary } else { t.text };
        let right = if own_hidden { bg.max.x - 26.0 } else { bg.max.x - 6.0 };
        if own_hidden {
            let badge = Rect::from_center_size(pos2(bg.max.x - 14.0, rect.center().y), vec2(ICON, ICON));
            icons::paint(ui.painter(), badge, Icon::EyeOff, t.icon_secondary);
        }
        crate::bidi::paint_name(ui.painter(), pos2(left, rect.center().y), &item.name, regular(FONT_SIZE), colour, (right - left).max(20.0));
        if resp.hovered() || resp.dragged() {
            self.wanted.push((src.clone(), item.id));
        }
        let imported = matches!(src, Source::Imported(_));
        let resp = super::tile_actions(ui, resp, src, item, &mut self.act);
        if resp.clicked() && imported {
            self.act = Some(Act::Show(item.id));
        }
    }

    /// A style's row: a click applies it to the selection.
    fn style(&mut self, ui: &mut Ui, src: &Source, style: &StyleRow) {
        let t = Tokens::get(ui.ctx());
        let (rect, resp) = ui.allocate_exact_size(vec2(self.w, ROW), Sense::click());
        let bg = rect.shrink2(vec2(SIDE, 0.0));
        if resp.hovered() {
            ui.painter().rect_filled(bg, CornerRadius::same(RADIUS), t.hover);
        }
        a11y::ring_around(ui, bg, &resp);
        let x = rect.min.x + indent(2);
        style_icon(ui, Rect::from_center_size(pos2(x + ICON / 2.0, rect.center().y), vec2(ICON, ICON)), &style.kind, t.text_secondary);
        let left = x + ICON + ICON_GAP;
        crate::bidi::paint_name(
            ui.painter(),
            pos2(left, rect.center().y),
            &style.name,
            regular(FONT_SIZE),
            t.text,
            (bg.max.x - 6.0 - left).max(20.0),
        );
        if resp.on_hover_text(crate::i18n::trf("{name}\nClick to apply to the selection", &[("name", &style.name)])).clicked() {
            self.act = Some(Act::Style(src.clone(), style.id.clone()));
        }
    }
}

/// The title row: chevron, name, then the search and libraries buttons (on hover or keyboard focus)
/// and the `+` that opens the source libraries. Its context menu offers the import window.
fn title_row(app: &mut FigApp, ui: &mut Ui, w: f32, uid: u64) -> bool {
    let t = Tokens::get(ui.ctx());
    let name = tr("Library|section");
    let open = app.ui.library_panel.is_open(uid, LIBRARY);
    let (rect, _) = ui.allocate_exact_size(vec2(w, TITLE), Sense::hover());
    let toggle_rect = Rect::from_min_max(rect.min, pos2(rect.max.x - 84.0, rect.max.y));
    let resp = ui.interact(toggle_rect, Id::unique("library_title"), Sense::click());
    a11y::toggle(&resp, name, open);
    a11y::ring_around(ui, toggle_rect.shrink2(vec2(SIDE, 0.0)), &resp);
    if resp.clicked() {
        app.ui.library_panel.toggle(uid, LIBRARY);
    }
    let x = rect.min.x + INNER;
    paint_chevron(ui, x, rect.center().y, open, t.text);
    ui.painter().text(pos2(x + ICON + ICON_GAP, rect.center().y), Align2::LEFT_CENTER, name, semibold(FONT_SIZE), t.text);
    let at = |offset: f32| Rect::from_center_size(pos2(rect.max.x - INNER - 7.0 - offset, rect.center().y), vec2(24.0, 24.0));
    let over = ui.rect_contains_pointer(rect);
    if title_button(ui, at(0.0), Id::unique("library_add"), tr("Add a library"), Icon::Plus, true, false).clicked() {
        app.run("project.settings", json!({}));
    }
    let search = app.ui.library_panel.search;
    let search_resp = title_button(ui, at(24.0), Id::unique("library_search"), tr("Search assets"), Icon::Search, over, search);
    if search_resp.clicked() {
        app.ui.library_panel.search = !search;
        app.ui.library_panel.filter.clear();
        app.ui.library_panel.folds.entry(uid).or_default().insert(LIBRARY.to_string(), true);
    }
    if title_button(ui, at(48.0), Id::unique("library_libraries"), tr("Libraries"), Icon::Library, over, false).clicked() {
        app.ui.libraries = true;
    }
    if imported::can_import(app) {
        let mut import = false;
        crate::right::context_popup(&resp).show(|ui| {
            if crate::right::menu_item(ui, tr("Import a component…"), "") {
                import = true;
                ui.close();
            }
        });
        if import {
            app.run(IMPORT_WINDOW, json!({}));
        }
    }
    search_resp.clicked()
}

/// The rows of the dropdown; folds it toggles go straight to the panel's state.
fn rows(app: &mut FigApp, ui: &mut Ui, list: &mut List, listing: &Listing, uid: u64, searching: bool) {
    let w = list.w;
    let mut toggled: Option<String> = None;
    let open = |app: &FigApp, key: &str| searching || app.ui.library_panel.is_open(uid, key);

    let local: Vec<&Tile> = listing.local.iter().flat_map(|folder| folder.tiles.iter()).collect();
    let is_open = open(app, LOCAL);
    if fold_row(ui, w, Id::unique("library_local"), tr("Local components"), local.len(), None, is_open).clicked() {
        toggled = Some(LOCAL.to_string());
    }
    if is_open {
        if local.is_empty() {
            if searching {
                empty_note(ui, w, tr("No matching components"));
            } else {
                let keys = crate::toolbar::pretty_shortcut(crate::shortcuts::shortcut("component.create").unwrap_or(""));
                empty_note(ui, w, &crate::i18n::trf("No components yet. Select layers and press {keys}.", &[("keys", &keys)]));
            }
        }
        for tile in local {
            list.component(ui, &Source::Local, tile);
        }
    }
    let is_open = open(app, STYLES);
    if fold_row(ui, w, Id::unique("library_styles"), tr("Local styles"), listing.local_styles.len(), None, is_open).clicked() {
        toggled = Some(STYLES.to_string());
    }
    if is_open {
        if listing.local_styles.is_empty() {
            empty_note(ui, w, if searching { tr("No matching styles") } else { tr("No styles yet.") });
        }
        for style in &listing.local_styles {
            list.style(ui, &Source::Local, style);
        }
    }
    for row in &listing.families {
        let is_open = open(app, &row.key);
        let note = row.removed.then(|| tr("source removed"));
        let resp = fold_row(ui, w, Id::unique(("library_family", &row.key)), &row.label, row.tiles.len(), note, is_open);
        if resp.clicked() {
            toggled = Some(row.key.clone());
        }
        if !row.removed && imported::can_import(app) {
            let over = ui.rect_contains_pointer(resp.rect);
            let button = Rect::from_center_size(pos2(resp.rect.max.x - SIDE - 30.0, resp.rect.center().y), vec2(24.0, 24.0));
            let id = Id::unique(("imported_import", &row.key));
            if title_button(ui, button, id, tr("Import a component…"), Icon::Plus, over, false).clicked() {
                list.act = Some(Act::ImportWindow(Some(row.package.clone())));
                toggled = None;
            }
        }
        if !is_open {
            continue;
        }
        if row.tiles.is_empty() {
            empty_note(ui, w, tr("Nothing imported from this source yet."));
        }
        for (src, tile) in &row.tiles {
            list.component(ui, src, tile);
        }
    }
    for section in &listing.libraries {
        let tiles: Vec<&Tile> = section.tiles.iter().flat_map(|folder| folder.tiles.iter()).collect();
        let key = format!("enabled/{}", section.id);
        let is_open = open(app, &key);
        if fold_row(ui, w, Id::unique(("library_enabled", &section.id)), &section.name, tiles.len(), None, is_open).clicked() {
            toggled = Some(key);
        }
        if !is_open {
            continue;
        }
        if let Some(error) = &section.error {
            empty_note(ui, w, &crate::i18n::trf("Can't read this library: {e}", &[("e", error)]));
        } else if section.doc.is_none() {
            empty_note(ui, w, tr("Reading the library…"));
        } else {
            let src = Source::Library(section.id.clone());
            for tile in tiles {
                list.component(ui, &src, tile);
            }
            for style in &section.styles {
                list.style(ui, &src, style);
            }
        }
    }
    if let Some(key) = toggled {
        app.ui.library_panel.toggle(uid, &key);
    }
}

/// Draw the dropdown as the first section under the header of a library tab.
pub(crate) fn section(app: &mut FigApp, ui: &mut Ui) {
    super::refresh_libraries(app, ui);
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let Ok(ds) = app.session.ds() else { return };
    let (doc, revision, uid, selection) = (ds.doc.clone(), ds.revision, ds.uid, ds.selection.clone());
    if std::mem::take(&mut app.ui.library_panel.reveal) {
        app.ui.library_panel.folds.entry(uid).or_default().insert(LIBRARY.to_string(), true);
    }
    ui.add_space(SECTION_PAD);
    let focus = title_row(app, ui, w, uid);
    if app.ui.library_panel.search {
        let panel = &mut app.ui.library_panel;
        crate::layers::search_field(ui, &t, w, Id::unique("asset_filter_field"), tr("Search assets"), &mut panel.filter, &mut panel.search, focus);
    }
    let q = app.ui.library_panel.filter.trim().to_lowercase();
    let listing = super::listing_of(app, ui, &doc, revision, &q);
    let thumbs = super::thumbs_for(ui, &doc, &listing);
    let mut list = List { w, doc: &doc, selection: &selection, wanted: Vec::new(), act: None };
    if listing.updates > 0 {
        super::updates_banner(app, ui, w, listing.updates);
    }
    if app.ui.library_panel.is_open(uid, LIBRARY) {
        let room = (ui.available_height() * 0.45).max(3.0 * ROW);
        egui::ScrollArea::vertical().id_salt("library_list").max_height(room).auto_shrink([false, true]).show(ui, |ui| {
            rows(app, ui, &mut list, &listing, uid, !q.is_empty());
        });
    }
    let (wanted, act) = (std::mem::take(&mut list.wanted), list.act.take());
    ui.add_space(SECTION_PAD);
    let (line, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().line_segment([line.left_center(), line.right_center()], egui::Stroke::new(1.0, t.border));
    super::finish(app, ui, w, &doc, &listing, thumbs, &wanted, act);
}
