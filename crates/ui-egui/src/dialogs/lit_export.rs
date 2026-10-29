//! File ▸ Export library as Lit…: plans the package the way `export.lit` does, lists the
//! components with their element name, lets the user check which ones to export, previews the
//! files and writes them to a folder. Programmatic `export.lit` calls never open it.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use egui::{Align2, Context, CornerRadius, RichText, Sense, vec2};
use figcraft_doc::{Document, NodeId};
use figcraft_engine::lit::{LitOptions, valid_prefix};
use figcraft_engine::lit_export::{ExportOptions, PlannedFile, Status, plan};
use serde_json::{Value, json};

use super::window;
use crate::FigApp;
use crate::i18n::{tr, trn};
use crate::theme::{Tokens, regular};

const WIDTH: f32 = 760.0;
const TREE_WIDTH: f32 = 230.0;
const LIST_HEIGHT: f32 = 150.0;
const FILES_HEIGHT: f32 = 220.0;
const NOTES_HEIGHT: f32 = 90.0;
const MAX_PREVIEW_CHARS: usize = 40_000;

#[derive(Clone, Debug)]
pub struct ComponentRow {
    pub id: NodeId,
    pub name: String,
    pub tag: String,
    pub status: String,
    pub checked: bool,
    pub issues: Vec<String>,
    messages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Settings {
    prefix: String,
    token_prefix: String,
    light: String,
    dark: String,
}

enum Setting {
    Prefix(String),
    TokenPrefix(String),
    Light(String),
    Dark(String),
}

enum Act {
    Toggle(NodeId, bool),
    Select(String),
    Set(Setting),
    Export,
    Close,
}

#[derive(Clone, Debug)]
pub struct LitExport {
    settings: Settings,
    unchecked: BTreeSet<NodeId>,
    rows: Vec<ComponentRow>,
    files: Vec<PlannedFile>,
    notes: Vec<String>,
    ready: bool,
    error: Option<String>,
    selected: Option<String>,
    seen: Option<(u64, Arc<Document>)>,
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Ok => "ok",
        Status::Empty => "empty",
        Status::Invalid => "invalid",
        Status::Dependency => "dependency",
    }
}

fn non_empty(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.to_string())
}

impl LitExport {
    fn new() -> Self {
        Self {
            settings: Settings { prefix: LitOptions::default().prefix, token_prefix: String::new(), light: String::new(), dark: String::new() },
            unchecked: BTreeSet::new(),
            rows: Vec::new(),
            files: Vec::new(),
            notes: Vec::new(),
            ready: false,
            error: None,
            selected: None,
            seen: None,
        }
    }

    pub fn components(&self) -> &[ComponentRow] {
        &self.rows
    }

    pub fn ids(&self) -> Vec<NodeId> {
        self.rows.iter().filter(|r| r.checked).map(|r| r.id).collect()
    }

    pub fn files(&self) -> Vec<&str> {
        self.files.iter().map(|f| f.path.as_str()).collect()
    }

    pub fn notes(&self) -> Vec<&str> {
        self.notes.iter().map(String::as_str).collect()
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn preview(&self) -> Option<&str> {
        let path = self.selected.as_deref()?;
        self.files.iter().find(|f| f.path == path).map(|f| f.text.as_str())
    }

    pub fn can_export(&self) -> bool {
        self.ready && self.rows.iter().any(|r| r.checked)
    }

    pub fn select(&mut self, path: &str) {
        if self.files.iter().any(|f| f.path == path) {
            self.selected = Some(path.to_string());
        }
    }

    fn options(&self, ids: Option<Vec<NodeId>>) -> ExportOptions {
        let mut lit = LitOptions::new(&self.settings.prefix);
        if !self.settings.token_prefix.is_empty() {
            lit.token_prefix.clone_from(&self.settings.token_prefix);
        }
        ExportOptions { ids, lit, light: non_empty(&self.settings.light), dark: non_empty(&self.settings.dark), ..ExportOptions::default() }
    }

    fn check(&mut self, id: NodeId, on: bool) {
        if on {
            self.unchecked.remove(&id);
        } else {
            self.unchecked.insert(id);
        }
        self.seen = None;
    }

    fn follows(&self, uid: u64, doc: &Arc<Document>) -> bool {
        self.seen.as_ref().is_some_and(|(u, d)| *u == uid && Arc::ptr_eq(d, doc))
    }

    fn replan(&mut self, uid: u64, doc: &Arc<Document>) {
        let all = plan(doc, &self.options(None));
        let wanted: Vec<NodeId> =
            all.components.iter().filter(|c| c.status != Status::Empty && !self.unchecked.contains(&c.id)).map(|c| c.id).collect();
        let chosen = plan(doc, &self.options(Some(wanted.clone())));
        self.rows = all
            .components
            .iter()
            .map(|c| {
                let shown = chosen.components.iter().find(|k| k.id == c.id).unwrap_or(c);
                ComponentRow {
                    id: c.id,
                    name: c.name.clone(),
                    tag: shown.tag.clone().unwrap_or_default(),
                    status: status_name(shown.status).to_string(),
                    checked: wanted.contains(&c.id),
                    issues: shown.issues.iter().map(|i| i.code.to_string()).collect(),
                    messages: shown.issues.iter().map(|i| i.message.clone()).collect(),
                }
            })
            .collect();
        self.ready = chosen.ready();
        self.notes = chosen.notes.iter().map(|n| n.message.clone()).collect();
        self.files = chosen.files;
        if !self.selected.as_ref().is_some_and(|s| self.files.iter().any(|f| &f.path == s)) {
            self.selected = self.files.first().map(|f| f.path.clone());
        }
        self.seen = Some((uid, Arc::clone(doc)));
    }

    fn apply(&mut self, setting: Setting) {
        let prefix = match &setting {
            Setting::Prefix(text) | Setting::TokenPrefix(text) => Some(text.as_str()),
            Setting::Light(_) | Setting::Dark(_) => None,
        };
        if prefix.is_some_and(|text| !valid_prefix(text)) {
            self.error = Some(tr("Lowercase letters, digits and hyphens, up to 20 characters").to_string());
            return;
        }
        self.error = None;
        match setting {
            Setting::Prefix(text) => self.settings.prefix = text,
            Setting::TokenPrefix(text) => self.settings.token_prefix = text,
            Setting::Light(text) => self.settings.light = text,
            Setting::Dark(text) => self.settings.dark = text,
        }
    }

    fn write_params(&self) -> Value {
        let mut params = json!({"ids": self.ids(), "prefix": self.settings.prefix});
        for (key, value) in [("tokenPrefix", &self.settings.token_prefix), ("lightMode", &self.settings.light), ("darkMode", &self.settings.dark)] {
            if !value.is_empty() {
                params[key] = json!(value);
            }
        }
        params
    }
}

/// `export.litWindow`: open the window for the file in front.
pub(crate) fn request(app: &mut FigApp) {
    let Ok(ds) = app.session.ds() else { return };
    let mut state = LitExport::new();
    state.replan(ds.uid, &ds.doc);
    app.ui.lit_export = Some(state);
}

/// Check or uncheck a component of the open window; the plan is made again.
pub fn set_checked(app: &mut FigApp, id: NodeId, checked: bool) {
    let (Some(state), Ok(ds)) = (app.ui.lit_export.as_mut(), app.session.ds()) else { return };
    state.check(id, checked);
    state.replan(ds.uid, &ds.doc);
}

fn written_note(app: &mut FigApp, result: &Value) {
    let count = |key: &str| result.get(key).and_then(Value::as_array).map_or(0, Vec::len);
    let summary =
        [trn("{n} written", count("written"), &[]), trn("{n} unchanged", count("unchanged"), &[]), trn("{n} skipped", count("skipped"), &[])];
    app.toast(summary.join(", "));
    let notes: Vec<String> = result
        .get("notes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|n| n.as_str().or_else(|| n.get("message").and_then(Value::as_str)).map(str::to_string))
        .collect();
    if let Some(state) = app.ui.lit_export.as_mut() {
        state.notes = notes;
    }
}

fn export(app: &mut FigApp, dir: PathBuf) {
    let Some(state) = &app.ui.lit_export else { return };
    let mut params = state.write_params();
    params["path"] = json!(dir.to_string_lossy());
    if let Some(result) = app.run("export.lit", params) {
        written_note(app, &result);
    }
}

/// Plan again when the document in front is not the one the plan was made for.
fn follow(app: &FigApp, state: &mut LitExport) -> bool {
    let Ok(ds) = app.session.ds() else { return false };
    if !state.follows(ds.uid, &ds.doc) {
        state.replan(ds.uid, &ds.doc);
    }
    true
}

pub fn show(app: &mut FigApp, ctx: &Context) {
    let Some(mut state) = app.ui.lit_export.take() else { return };
    if !follow(app, &mut state) {
        return;
    }
    let t = Tokens::get(ctx);
    let can_write = !app.services.downloads();
    let mut open = true;
    let mut acts = Vec::new();
    window(tr("Export library as Lit"), ctx).open(&mut open).show(ctx, |ui| {
        ui.set_width(WIDTH);
        acts = body(ui, &t, &state, can_write);
    });
    let mut export_asked = false;
    for act in acts {
        match act {
            Act::Toggle(id, on) => state.check(id, on),
            Act::Select(path) => state.select(&path),
            Act::Set(setting) => {
                state.apply(setting);
                state.seen = None;
            }
            Act::Export => export_asked = state.can_export(),
            Act::Close => open = false,
        }
    }
    if open && follow(app, &mut state) {
        app.ui.lit_export = Some(state);
        if export_asked {
            app.pick_folder(tr("Export package"), export);
        }
    }
}

fn body(ui: &mut egui::Ui, t: &Tokens, state: &LitExport, can_write: bool) -> Vec<Act> {
    let mut acts = Vec::new();
    settings_row(ui, t, &state.settings, &mut acts);
    if let Some(error) = &state.error {
        ui.label(RichText::new(error).color(t.danger).font(regular(11.0)));
    }
    ui.add_space(8.0);
    ui.label(RichText::new(tr("Components")).strong());
    components_list(ui, t, state, &mut acts);
    ui.add_space(8.0);
    ui.label(RichText::new(tr("Files")).strong());
    files_and_preview(ui, t, state, &mut acts);
    if !state.notes.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Notes")).strong());
        egui::ScrollArea::vertical().id_salt("lit_export_notes").max_height(NOTES_HEIGHT).auto_shrink([false, true]).show(ui, |ui| {
            for note in &state.notes {
                ui.label(RichText::new(note).color(t.text_secondary).font(regular(11.0)));
            }
        });
    }
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if crate::widgets::button(ui, tr("Close"), false, true).clicked() {
            acts.push(Act::Close);
        }
        if can_write && crate::widgets::button(ui, tr("Export package"), true, state.can_export()).clicked() {
            acts.push(Act::Export);
        }
    });
    acts
}

fn settings_row(ui: &mut egui::Ui, t: &Tokens, settings: &Settings, acts: &mut Vec<Act>) {
    type Make = fn(String) -> Setting;
    let fields: [(&str, &str, Make); 4] = [
        (tr("Element name prefix"), &settings.prefix, Setting::Prefix),
        (tr("Token prefix"), &settings.token_prefix, Setting::TokenPrefix),
        (tr("Light mode name"), &settings.light, Setting::Light),
        (tr("Dark mode name"), &settings.dark, Setting::Dark),
    ];
    ui.horizontal_top(|ui| {
        for (i, (label, value, make)) in fields.into_iter().enumerate() {
            ui.vertical(|ui| {
                ui.label(RichText::new(label).color(t.text_secondary).font(regular(11.0)));
                if let Some(text) = crate::widgets::text_field(ui, egui::Id::unique(("lit_export_setting", i)), value, 170.0, tr("Auto")) {
                    acts.push(Act::Set(make(text)));
                }
            });
        }
    });
}

fn components_list(ui: &mut egui::Ui, t: &Tokens, state: &LitExport, acts: &mut Vec<Act>) {
    if state.rows.is_empty() {
        ui.label(RichText::new(tr("This file has no components to export")).color(t.text_secondary));
        return;
    }
    egui::ScrollArea::vertical().id_salt("lit_export_components").max_height(LIST_HEIGHT).auto_shrink([false, true]).show(ui, |ui| {
        for row in &state.rows {
            let label = if row.tag.is_empty() { row.name.as_str() } else { row.tag.as_str() };
            ui.horizontal(|ui| {
                if row.status == "empty" {
                    ui.label(RichText::new(format!("{label} · {}", tr("Empty"))).color(t.text_tertiary).font(regular(11.0)));
                    return;
                }
                if crate::widgets::checkbox(ui, row.checked, label) {
                    acts.push(Act::Toggle(row.id, !row.checked));
                }
                if row.tag.as_str() != row.name.as_str() {
                    ui.label(RichText::new(&row.name).color(t.text_secondary).font(regular(11.0)));
                }
                if row.status == "dependency" {
                    ui.label(RichText::new(tr("Added because another component uses it")).color(t.text_secondary).font(regular(11.0)));
                }
            });
            for message in &row.messages {
                ui.label(RichText::new(message).color(t.danger).font(regular(11.0)));
            }
        }
    });
}

fn file_row(ui: &mut egui::Ui, t: &Tokens, name: &str, indent: f32, active: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, active, name));
    if active || resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), if active { t.selected_row } else { t.hover });
    }
    ui.painter().text(egui::pos2(rect.min.x + indent + 8.0, rect.center().y), Align2::LEFT_CENTER, name, regular(11.0), t.text);
    resp.clicked()
}

fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn files_and_preview(ui: &mut egui::Ui, t: &Tokens, state: &LitExport, acts: &mut Vec<Act>) {
    let preview_width = WIDTH - TREE_WIDTH - ui.spacing().item_spacing.x;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(TREE_WIDTH);
            egui::ScrollArea::vertical().id_salt("lit_export_files").max_height(FILES_HEIGHT).auto_shrink([false, true]).show(ui, |ui| {
                let mut folders: Vec<&str> = state.files.iter().map(|f| folder_of(&f.path)).filter(|d| !d.is_empty()).collect();
                folders.sort_unstable();
                folders.dedup();
                for folder in std::iter::once("").chain(folders) {
                    if !folder.is_empty() {
                        ui.label(RichText::new(format!("{folder}/")).color(t.text_secondary).font(regular(11.0)));
                    }
                    for file in state.files.iter().filter(|f| folder_of(&f.path) == folder) {
                        let name = file.path.rsplit('/').next().unwrap_or(&file.path);
                        let indent = if folder.is_empty() { 0.0 } else { 12.0 };
                        if file_row(ui, t, name, indent, state.selected.as_deref() == Some(file.path.as_str())) {
                            acts.push(Act::Select(file.path.clone()));
                        }
                    }
                }
            });
        });
        ui.vertical(|ui| {
            ui.set_width(preview_width);
            egui::ScrollArea::both().id_salt("lit_export_preview").max_height(FILES_HEIGHT).auto_shrink([false, true]).show(ui, |ui| {
                let Some(text) = state.preview() else { return };
                let shown: String = text.chars().take(MAX_PREVIEW_CHARS).collect();
                ui.add(egui::Label::new(RichText::new(shown).monospace().size(11.0).color(t.text)).wrap_mode(egui::TextWrapMode::Extend));
            });
        });
    });
}
