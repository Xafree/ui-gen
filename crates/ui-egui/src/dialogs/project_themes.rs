//! Project settings ▸ "Token themes": the variable collections of the library project, read from
//! the document (`token_themes`), with the one `uigen.json` calls applied. "Apply" runs
//! `uigen.applyTheme`; programmatic calls of it never open anything.

use std::fmt;
use std::sync::Arc;

use egui::{Align, Color32, CornerRadius, Frame, Label, Layout, Margin, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use figcraft_doc::Document;
use figcraft_engine::capture::JobState;
use figcraft_engine::{Swatch, ThemeInfo, file_access, token_themes};
use figcraft_project::Project;
use serde_json::{Value, json};

use super::project_settings::{CARD_MARGIN, GAP, INNER_W, note};
use crate::FigApp;
use crate::i18n::{tr, trf, trn};
use crate::theme::{Tokens, doc_c32, regular, semibold};

const APPLY: &str = "uigen.applyTheme";
const IMPORT: &str = "uigen.importComponent";
const LIST_HEIGHT: f32 = 340.0;
const SWATCH: f32 = 16.0;
const SWATCH_GAP: f32 = 4.0;
const FACT_GAP: f32 = 12.0;

#[derive(Clone)]
struct Anchor(Arc<Document>);

impl fmt::Debug for Anchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Anchor")
    }
}

#[derive(Clone, Debug)]
enum Outcome {
    Applied { theme: String, layers: usize, unmatched: Vec<String> },
    Failed { theme: String, cause: String },
}

impl Outcome {
    fn text(&self) -> String {
        match self {
            Self::Applied { theme, layers, .. } => trf("{theme} applied: {layers} layers follow it", &[("theme", theme), ("layers", layers)]),
            Self::Failed { theme, cause } => trf("Couldn't apply {theme}: {cause}", &[("theme", theme), ("cause", cause)]),
        }
    }
}

pub(super) enum ThemeAct {
    Apply(String),
    OpenVariables,
    ToggleUnmatched,
}

#[derive(Clone, Debug, Default)]
pub struct Themes {
    rows: Vec<ThemeInfo>,
    applied: Option<String>,
    importing: bool,
    outcome: Option<(Anchor, Outcome)>,
    unmatched_open: bool,
    seen: Option<(u64, u64, Option<String>)>,
}

impl Themes {
    pub(super) fn refresh(&mut self, app: &FigApp, root: &str, config: &Project) {
        self.importing = importing(app, root);
        let Ok(ds) = app.session.ds() else {
            self.rows.clear();
            self.seen = None;
            return;
        };
        if self.outcome.as_ref().is_some_and(|(anchor, _)| !Arc::ptr_eq(&anchor.0, &ds.doc)) {
            self.outcome = None;
        }
        let applied = config.applied_theme.clone();
        let key = (ds.uid, ds.revision, applied.clone());
        if self.seen.as_ref() == Some(&key) {
            return;
        }
        self.rows = token_themes(&ds.doc, config);
        self.applied = applied;
        self.seen = Some(key);
    }

    pub(super) fn invalidate(&mut self) {
        self.seen = None;
    }

    fn followed(&self) -> Option<&ThemeInfo> {
        let mut best: Option<&ThemeInfo> = None;
        for row in &self.rows {
            if row.bound_layers > best.map_or(0, |b| b.bound_layers) {
                best = Some(row);
            }
        }
        best
    }

    fn followers_of(&self, theme: &str) -> usize {
        self.rows.iter().find(|row| row.name == theme).map_or(0, |row| row.bound_layers)
    }
}

fn importing(app: &FigApp, root: &str) -> bool {
    app.session
        .capture_jobs()
        .iter()
        .any(|job| job.command == IMPORT && matches!(job.state, JobState::Queued | JobState::Running) && file_access::same_file(&job.root, root))
}

fn themes_mut(app: &mut FigApp) -> Option<&mut Themes> {
    app.ui.project_settings.as_mut().map(|state| &mut state.themes)
}

pub(super) fn perform(app: &mut FigApp, act: ThemeAct) {
    match act {
        ThemeAct::Apply(theme) => apply(app, &theme),
        ThemeAct::OpenVariables => {
            app.ui.variables.get_or_insert_with(Default::default);
        }
        ThemeAct::ToggleUnmatched => {
            if let Some(themes) = themes_mut(app) {
                themes.unmatched_open = !themes.unmatched_open;
            }
        }
    }
}

fn apply(app: &mut FigApp, theme: &str) {
    let outcome = match app.run(APPLY, json!({"theme": theme})) {
        Some(result) => {
            let outcome = applied(app, theme, &result);
            app.toast_with_undo(outcome.text());
            outcome
        }
        None => Outcome::Failed { theme: theme.to_owned(), cause: app.ui.last_error.clone().unwrap_or_default() },
    };
    let anchor = app.session.ds().ok().map(|ds| Anchor(Arc::clone(&ds.doc)));
    if let Some(themes) = themes_mut(app) {
        themes.outcome = anchor.map(|anchor| (anchor, outcome));
        themes.unmatched_open = false;
    }
}

fn applied(app: &FigApp, theme: &str, result: &Value) -> Outcome {
    let unchanged = result["unchanged"].as_bool() == Some(true);
    let layers = if unchanged {
        app.ui.project_settings.as_ref().map_or(0, |state| state.themes.followers_of(theme))
    } else {
        result["layers"].as_u64().and_then(|n| usize::try_from(n).ok()).unwrap_or_default()
    };
    let unmatched =
        result["unmatched"].as_array().map(|names| names.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default();
    Outcome::Applied { theme: theme.to_owned(), layers, unmatched }
}

pub(super) fn section(ui: &mut Ui, t: &Tokens, state: &Themes, acts: &mut Vec<ThemeAct>) {
    ui.label(RichText::new(tr("Token themes")).font(semibold(15.0)).color(t.text));
    ui.add_space(8.0);
    if state.importing {
        note(ui, tr("An import is running: Apply is available once it has finished."), t.text_secondary);
    }
    messages(ui, t, state, acts);
    if state.rows.is_empty() {
        note(ui, tr("No token theme yet. Import a component or tokens to create one."), t.text_secondary);
        ui.add_space(6.0);
        if crate::widgets::button(ui, tr("Open the Variables window"), false, true).clicked() {
            acts.push(ThemeAct::OpenVariables);
        }
        return;
    }
    ScrollArea::vertical().id_salt("project_settings_themes").max_height(LIST_HEIGHT).auto_shrink([false, true]).show(ui, |ui| {
        for row in &state.rows {
            card(ui, t, state, row, acts);
            ui.add_space(6.0);
        }
    });
}

fn messages(ui: &mut Ui, t: &Tokens, state: &Themes, acts: &mut Vec<ThemeAct>) {
    match state.outcome.as_ref().map(|(_, outcome)| outcome) {
        Some(outcome @ Outcome::Applied { theme, unmatched, .. }) => {
            note(ui, &outcome.text(), t.success);
            if !unmatched.is_empty() {
                let message = trf(
                    "{count} tokens have no equivalent in {theme} and keep their previous value",
                    &[("count", &unmatched.len()), ("theme", theme)],
                );
                if crate::widgets::disclosure(ui, &message, state.unmatched_open) {
                    acts.push(ThemeAct::ToggleUnmatched);
                }
                if state.unmatched_open {
                    note(ui, &unmatched.join(", "), t.text_secondary);
                }
            }
        }
        Some(outcome @ Outcome::Failed { .. }) => note(ui, &outcome.text(), t.danger),
        None => {}
    }
    if let Some(followed) = state.followed()
        && state.applied.as_deref() != Some(followed.name.as_str())
    {
        note(ui, &trf("The layers follow {theme}, not the applied theme. Apply it again to sync.", &[("theme", &followed.name)]), t.warning);
    }
    ui.add_space(6.0);
}

fn card(ui: &mut Ui, t: &Tokens, state: &Themes, row: &ThemeInfo, acts: &mut Vec<ThemeAct>) {
    let applied = state.applied.as_deref() == Some(row.name.as_str());
    Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(CARD_MARGIN)).show(ui, |ui| {
        ui.set_width(INNER_W);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            ui.add(Label::new(RichText::new(&row.name).font(semibold(12.0)).color(t.text)).truncate());
            if applied {
                ui.label(RichText::new(tr("Applied")).font(semibold(11.0)).color(t.accent_text));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if crate::widgets::button(ui, tr("Apply"), !applied, !state.importing).clicked() {
                    acts.push(ThemeAct::Apply(row.name.clone()));
                }
            });
        });
        facts(ui, t, row);
        if !row.swatches.is_empty() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = SWATCH_GAP;
                for swatch in &row.swatches {
                    swatch_chip(ui, t, swatch);
                }
            });
        }
    });
}

fn facts(ui: &mut Ui, t: &Tokens, row: &ThemeInfo) {
    let fact = |ui: &mut Ui, text: String| {
        ui.label(RichText::new(text).font(regular(11.0)).color(t.text_secondary));
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = FACT_GAP;
        if let Some((package, version)) = &row.origin {
            fact(ui, trf("Imported from {package} {version}", &[("package", package), ("version", version)]));
        }
        fact(ui, row.modes.join(", "));
        fact(ui, trn("{n} variables", row.variables, &[]));
        fact(ui, trf("{count} layers use it", &[("count", &row.bound_layers)]));
    });
}

fn swatch_chip(ui: &mut Ui, t: &Tokens, swatch: &Swatch) {
    let (rect, resp) = ui.allocate_exact_size(vec2(SWATCH, SWATCH), Sense::hover());
    let value = format!("{} {}, {} {}", tr("Light"), swatch.light, tr("Dark"), swatch.dark);
    crate::a11y::color(&resp, &swatch.name, &value);
    let colour = |hex: &str| figcraft_color::Color::from_hex(hex).map_or(Color32::TRANSPARENT, |c| doc_c32(ui.ctx(), c));
    let radius = 4;
    let (light, dark) = (colour(&swatch.light), colour(&swatch.dark));
    let left = egui::Rect::from_min_max(rect.min, pos2(rect.center().x, rect.max.y));
    let right = egui::Rect::from_min_max(pos2(rect.center().x, rect.min.y), rect.max);
    ui.painter().rect_filled(left, CornerRadius { nw: radius, sw: radius, ne: 0, se: 0 }, light);
    ui.painter().rect_filled(right, CornerRadius { nw: 0, sw: 0, ne: radius, se: radius }, dark);
    ui.painter().rect_stroke(rect, CornerRadius::same(radius), Stroke::new(1.0, t.border_strong), StrokeKind::Inside);
    resp.on_hover_text(format!("{}  {value}", swatch.name));
}
