//! The "Imported · <package> <version>" sections of the Components tab: one per source of the
//! library project, after the local styles. A section lists the component sets of its import page as
//! tiles (their thumbnails come from the file like the local ones), folds like a library, and its
//! tiles offer "Copy to the current page" and "Show in the import page" from their context menu,
//! with the pointer or the keyboard (Shift+F10, the menu key). A button asks for the import window.

use std::collections::HashSet;

use egui::{CornerRadius, Id, Key, Modifiers, Rect, Response, Sense, Ui, pos2, vec2};
use figcraft_doc::{Document, NodeId};
use figcraft_engine::imported_sections;

use super::{Act, PAD, Source, Thumbs, Tile, hint, matches};
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
    pub title: String,
    pub removed: bool,
    pub tiles: Vec<Tile>,
}

/// What the file holds from imports: the sections to show and the pages they come from (whose
/// components are not part of the library).
#[derive(Clone, Default)]
pub(super) struct Imported {
    pub blocks: Vec<Block>,
    pages: HashSet<NodeId>,
}

impl Imported {
    pub fn holds(&self, doc: &Document, component: NodeId) -> bool {
        doc.page_of(component).is_some_and(|page| self.pages.contains(&page))
    }
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
        found.pages.extend(section.page);
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
        found.blocks.push(Block { key, package: section.package, title, removed: section.removed, tiles });
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
    if title_row(ui, w, block, open, importable, act) {
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
