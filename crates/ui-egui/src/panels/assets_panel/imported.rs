//! The "Imported · <package> <version>" sections of the Components tab: one per source of the
//! library project, after the local styles. A section lists the component sets of its import page as
//! tiles (their thumbnails come from the file like the local ones), folds like a library, and its
//! tiles offer "Copy to the current page" and "Show in the import page" from their context menu,
//! with the pointer or the keyboard (Shift+F10, the menu key). A tile also drags onto the canvas,
//! where releasing it makes the set the component of the open page. A button asks for the import window.

use std::collections::HashSet;

use egui::{CornerRadius, Id, Key, Modifiers, Pos2, Rect, Response, Sense, Ui, pos2, vec2};
use figcraft_doc::{Document, NodeId};
use figcraft_engine::project::component_roots;
use figcraft_engine::{imported_sections, is_import_page};
use serde_json::{Value, json};

use super::{Act, PAD, PLACE_IMPORTED, Source, Thumbs, Tile, hint, matches};
use crate::FigApp;
use crate::a11y::Tip;
use crate::i18n::{tr, trf};
use crate::icons::{self, Icon};
use crate::theme::{Tokens, regular, semibold};

const BUTTON: f32 = 24.0;

/// One section as listed: the sets of its import page that match the search.
#[derive(Clone)]
pub(super) struct Block {
    /// Keys the thumbnails of the section's tiles (`<package> <version>`) and remembers its fold.
    pub key: String,
    pub package: String,
    pub version: String,
    pub title: String,
    pub removed: bool,
    pub tiles: Vec<Tile>,
}

/// What the file holds from imports: the sections to show.
#[derive(Clone, Default)]
pub(super) struct Imported {
    pub blocks: Vec<Block>,
}

/// Whether the panel offers the import window: in a library project, on the desktop.
pub(super) fn can_import(app: &FigApp) -> bool {
    !cfg!(target_arch = "wasm32") && app.session.in_lib_project()
}

/// The sections of `doc` for the library project of the tab (none outside a project); while
/// searching, those with no set matching `q` stay out of the way.
pub(super) fn list(app: &FigApp, doc: &Document, q: &str) -> Imported {
    let Some(binding) = app.session.project() else { return Imported::default() };
    let mut found = Imported::default();
    for section in imported_sections(doc, &binding.config) {
        let tiles: Vec<Tile> = section
            .components
            .iter()
            .filter(|set| matches(&set.name, q))
            .map(|set| Tile { id: set.set, name: set.name.clone(), label: set.name.clone(), set: true, hidden: false })
            .collect();
        if !q.is_empty() && tiles.is_empty() {
            continue;
        }
        let key = format!("{} {}", section.package, section.version);
        let title = trf("Imported · {package} {version}", &[("package", &section.package), ("version", &section.version)]);
        found.blocks.push(Block { key, package: section.package, version: section.version, title, removed: section.removed, tiles });
    }
    found
}

pub(super) fn fold_key(block: &Block) -> String {
    format!("imported/{}", block.key)
}

/// A section: its title row (a click folds it) and its tiles, or the note saying nothing is imported.
pub(super) fn section(
    ui: &mut Ui,
    w: f32,
    block: &Block,
    searching: bool,
    collapsed: &HashSet<String>,
    importable: bool,
    thumbs: &Thumbs,
    wanted: &mut Vec<(Source, NodeId)>,
    act: &mut Option<Act>,
) {
    let open = searching || !collapsed.contains(&fold_key(block));
    if title_row(ui, w, block, open, importable && !block.removed, act) {
        *act = Some(Act::Toggle(fold_key(block)));
    }
    if !open {
        return;
    }
    if block.tiles.is_empty() {
        hint(ui, tr("Nothing imported from this source yet."));
        return;
    }
    super::tiles(ui, w, &Source::Imported(block.key.clone()), &block.tiles, thumbs, wanted, act);
}

/// The title row with its chevron, "source removed" when the source is gone and the import button;
/// returns whether the row itself was clicked.
fn title_row(ui: &mut Ui, w: f32, block: &Block, open: bool, importable: bool, act: &mut Option<Act>) -> bool {
    let t = Tokens::get(ui.ctx());
    let (row, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
    let chevron = Rect::from_center_size(pos2(row.min.x + PAD + 5.0, row.center().y), vec2(12.0, 12.0));
    icons::paint(ui.painter(), chevron, if open { Icon::Chevron } else { Icon::ChevronRight }, t.icon_secondary);
    let button = importable.then(|| Rect::from_center_size(pos2(row.max.x - 20.0, row.center().y), vec2(BUTTON, BUTTON)));
    let right = button.map_or(row.max.x - PAD, |b| b.min.x - 4.0);
    let note = block.removed.then(|| ui.painter().layout_no_wrap(tr("source removed").to_string(), regular(10.0), t.text_secondary));
    let note_w = note.as_ref().map_or(0.0, |note| note.size().x + 8.0);
    let left = chevron.max.x + 6.0;
    let title =
        crate::bidi::paint_name(ui.painter(), pos2(left, row.center().y), &block.title, semibold(11.0), t.text, (right - left - note_w).max(40.0));
    if let Some(note) = note {
        ui.painter().galley(pos2(title.max.x + 8.0, row.center().y - note.size().y / 2.0), note, t.text_secondary);
    }
    let mut clicked = resp.on_hover_text(if open { tr("Library: click to collapse") } else { tr("Library: click to expand") }).clicked();
    if let Some(button) = button
        && import_button(ui, button, Id::unique(("imported_import", &block.key)))
    {
        *act = Some(Act::ImportWindow(Some(block.package.clone())));
        clicked = false;
    }
    clicked
}

/// The "Import a component…" button in `rect`; returns whether it was clicked.
pub(super) fn import_button(ui: &mut Ui, rect: Rect, id: Id) -> bool {
    let t = Tokens::get(ui.ctx());
    let resp = ui.interact(rect, id, Sense::click()).tip(tr("Import a component…"));
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), t.hover);
    }
    icons::paint(ui.painter(), rect.shrink(5.0), Icon::Plus, t.icon);
    resp.clicked()
}

/// The context menu of an imported tile (secondary click, Shift+F10 or the menu key on the focused
/// tile): copy the set to the current page, or show it in its import page.
pub(super) fn tile_menu(ui: &Ui, resp: &Response, set: NodeId, act: &mut Option<Act>) {
    let ctx = ui.ctx();
    let from_keyboard = resp.has_focus() && ui.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::F10) || i.consume_key(Modifiers::NONE, Key::Menu));
    let anchored = resp.id.with("menu_below_tile");
    let open = egui::Popup::is_id_open(ctx, egui::Popup::default_response_id(resp));
    let below = from_keyboard || (open && ctx.data(|d| d.get_temp::<bool>(anchored)).unwrap_or(false));
    ctx.data_mut(|d| d.insert_temp(anchored, below));
    let mut popup = crate::right::context_popup(resp);
    if below {
        popup = popup.anchor(resp.rect);
    }
    if from_keyboard {
        popup = popup.open_memory(egui::SetOpenCommand::Bool(true));
    }
    popup.show(|ui| {
        if crate::right::menu_item(ui, tr("Copy to the current page"), "") {
            *act = Some(Act::Copy(set));
            ui.close();
        }
        if crate::right::menu_item(ui, tr("Show in the import page"), "") {
            *act = Some(Act::Show(set));
            ui.close();
        }
    });
}

/// The tooltip of an imported tile, which also names it for screen readers.
pub(super) fn tile_tip(resp: Response, name: &str) -> Response {
    resp.tip(trf("{name}\nDrag onto the canvas to copy it into your library.", &[("name", &name)]))
}

/// What releasing an imported tile on `page` would do; a page the engine refuses gets no hint.
fn hint_text(doc: &Document, page: Option<NodeId>) -> Option<String> {
    let page = page.filter(|page| !doc.is_divider(*page) && !is_import_page(doc, *page))?;
    let node = doc.node(page)?;
    Some(if component_roots(doc, page).is_empty() {
        tr("Drop to make it the component of this page").to_string()
    } else {
        trf("Drop to replace {name} on this page", &[("name", &node.name)])
    })
}

/// What releasing an imported tile over the canvas would do to `page`, painted under the ghost at `p`.
pub(super) fn drop_hint(ui: &Ui, doc: &Document, page: Option<NodeId>, p: Pos2) {
    let Some(text) = hint_text(doc, page) else { return };
    let t = Tokens::get(ui.ctx());
    let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::unique("asset_drop_hint")));
    let galley = painter.layout_no_wrap(text, regular(11.0), t.text);
    let size = galley.size() + vec2(16.0, 10.0);
    let screen = ui.ctx().content_rect();
    let left = (p.x - size.x / 2.0).clamp(screen.min.x, (screen.max.x - size.x).max(screen.min.x));
    let top = (p.y + 40.0).min((screen.max.y - size.y).max(screen.min.y));
    let pill = Rect::from_min_size(pos2(left, top), size);
    painter.rect_filled(pill, CornerRadius::same(6), t.panel.gamma_multiply(0.95));
    painter.rect_stroke(pill, CornerRadius::same(6), egui::Stroke::new(1.0, crate::theme::COMPONENT), egui::StrokeKind::Inside);
    painter.galley(pill.center() - galley.size() / 2.0, galley, t.text);
}

/// The drop of an imported set on the canvas of `page`: place it, then say what happened with an
/// "Undo" (the toast comes after the command, so its action holds the revision the placement made).
pub(super) fn drop_on_canvas(app: &mut FigApp, set: NodeId, page: Option<NodeId>) {
    if crate::menus::dev_mode_refuses(app, PLACE_IMPORTED) {
        return;
    }
    let mut params = json!({"set": set.0});
    if let Some(page) = page {
        params["page"] = json!(page.0);
    }
    let Some(result) = app.run(PLACE_IMPORTED, params) else { return };
    let name = |value: &Value| value.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
    let message = match result.get("replaced").filter(|replaced| !replaced.is_null()) {
        Some(replaced) => trf("{old} replaced by {new}", &[("old", &name(replaced)), ("new", &name(&result))]),
        None => trf("Copied {name} into your library", &[("name", &name(&result))]),
    };
    app.toast_with_undo(message);
}
