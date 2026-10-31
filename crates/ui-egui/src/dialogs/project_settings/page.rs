use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Frame, Galley, Label, Layout, Margin, Rect, Response, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, TextEdit, Ui, UiBuilder, pos2, vec2,
};

use super::{Act, FamilyRow, ProjectSettings, RunState, Work, failure_text, package_field, project_themes};
use crate::a11y;
use crate::i18n::{tr, trf, trn};
use crate::theme::{Tokens, regular, semibold};

pub(in crate::dialogs) const MAX_WIDTH: f32 = 1120.0;
pub(in crate::dialogs) const BAR_H: f32 = 52.0;
pub(in crate::dialogs) const CARD_RADIUS: u8 = 14;
const ROW_PAD_X: f32 = 16.0;
const ROW_PAD_Y: f32 = 12.0;
const ACTIONS_H: f32 = 32.0;
const TAG_ROW_H: f32 = 23.0;
const HEAD_H: f32 = 42.0;
pub(in crate::dialogs) const BODY_LINE: f32 = 1.45;
const READY_TEXT_BASIS: f32 = 240.0;
const MONO_LINE: f32 = 14.0;
const EXAMPLES_LINE: f32 = 19.0;
const COLUMNS: usize = 5;
const EXAMPLES: [&str; 2] = ["npm install @heroui/react", "npm i @mui/material @emotion/react @emotion/styled"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::dialogs) enum Look {
    Secondary,
    Primary,
    Ghost,
    Icon,
}

pub(in crate::dialogs) fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub(in crate::dialogs) fn text_width(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), font, Color32::WHITE).size().x
}

#[derive(Clone, Default)]
struct FocusTrack {
    last: Option<egui::Id>,
    pointer_pass: u64,
    keyboard: bool,
}

fn focus_track_id() -> egui::Id {
    egui::Id::unique("project_page_focus")
}

pub(in crate::dialogs) fn track_keyboard_focus(ui: &Ui) {
    let ctx = ui.ctx();
    let pass = ctx.cumulative_pass_nr();
    let focused = ctx.memory(|m| m.focused());
    let pressed = ui.input(|i| i.pointer.any_pressed());
    ctx.data_mut(|data| {
        let track = data.get_temp_mut_or_default::<FocusTrack>(focus_track_id());
        if pressed {
            track.pointer_pass = pass;
            track.keyboard = false;
        }
        if focused != track.last {
            if track.last.is_some() && focused.is_some() && pass > track.pointer_pass + 1 {
                track.keyboard = true;
            }
            track.last = focused;
        }
    });
}

pub(in crate::dialogs) fn keyboard_focus(ui: &Ui) -> bool {
    ui.data(|data| data.get_temp::<FocusTrack>(focus_track_id()).is_some_and(|track| track.keyboard))
}

/// egui drops the focus on Escape before anything runs, so a field remembers it had it.
pub(in crate::dialogs) fn note_focus(ui: &Ui, field: egui::Id, focused: bool) {
    ui.data_mut(|data| data.insert_temp(field.with("had_focus"), focused));
}

pub(in crate::dialogs) fn had_focus(ctx: &egui::Context, field: egui::Id) -> bool {
    ctx.data(|data| data.get_temp::<bool>(field.with("had_focus"))).unwrap_or(false)
}

pub(in crate::dialogs) fn focus_ring(ui: &Ui, t: &Tokens, rect: Rect, radius: u8) {
    if !keyboard_focus(ui) {
        return;
    }
    ui.painter().rect_stroke(rect.expand(2.0), CornerRadius::same(radius + 2), Stroke::new(2.0, t.accent_text), StrokeKind::Outside);
}

pub(in crate::dialogs) fn button_at(ui: &mut Ui, t: &Tokens, rect: Rect, label: &str, look: Look, enabled: bool) -> Response {
    let resp = ui.interact(
        rect,
        ui.scope_id().with(("page_button", label, rect.min.x as i32, rect.min.y as i32)),
        if enabled { Sense::click() } else { Sense::hover() },
    );
    a11y::button(&resp, label);
    let hovered = enabled && resp.hovered();
    let ghost = matches!(look, Look::Ghost | Look::Icon);
    let radius = if ghost { 8 } else { 9 };
    let (fill, ink) = match (look, enabled) {
        (Look::Primary, true) => (t.accent, t.accent_on),
        (Look::Primary, false) => (t.field, t.text_tertiary),
        (Look::Secondary, true) => (if hovered { t.hover } else { t.bg }, t.text),
        (Look::Secondary, false) => (t.bg, t.text_tertiary),
        (_, true) => (if hovered { t.hover } else { Color32::TRANSPARENT }, t.text_secondary),
        (_, false) => (Color32::TRANSPARENT, t.text_tertiary),
    };
    ui.painter().rect_filled(rect, CornerRadius::same(radius), fill);
    if look == Look::Secondary {
        ui.painter().rect_stroke(rect, CornerRadius::same(radius), Stroke::new(1.0, t.border), StrokeKind::Inside);
    }
    if look == Look::Icon {
        let stroke = Stroke::new(1.5, ink);
        let c = rect.center();
        ui.painter().line_segment([c + vec2(-4.5, -4.5), c + vec2(4.5, 4.5)], stroke);
        ui.painter().line_segment([c + vec2(-4.5, 4.5), c + vec2(4.5, -4.5)], stroke);
    } else {
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, regular(13.5), ink);
    }
    if resp.has_focus() {
        focus_ring(ui, t, rect, radius);
    }
    resp
}

pub(in crate::dialogs) fn button(ui: &mut Ui, t: &Tokens, label: &str, look: Look, height: f32, pad: f32, enabled: bool) -> Response {
    let width = text_width(ui, label, regular(13.5)) + 2.0 * pad;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    button_at(ui, t, rect, label, look, enabled)
}

pub(in crate::dialogs) fn tag_width(ui: &Ui, text: &str) -> f32 {
    text_width(ui, text, regular(11.5)) + 16.0
}

pub(in crate::dialogs) fn tag_at(ui: &Ui, left: f32, centre_y: f32, text: &str, bg: Color32, ink: Color32) {
    let rect = Rect::from_min_size(pos2(left, centre_y - 11.0), vec2(tag_width(ui, text), 22.0));
    ui.painter().rect_filled(rect, CornerRadius::same(11), bg);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, regular(11.5), ink);
}

pub(in crate::dialogs) fn card_frame(t: &Tokens, padding: i8) -> Frame {
    Frame::new().fill(t.bg).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(CARD_RADIUS)).inner_margin(Margin::same(padding))
}

fn heading(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.label(RichText::new(text).font(semibold(16.0)).color(t.text).line_height(Some(16.0 * BODY_LINE)));
}

pub(super) fn show(ui: &mut Ui, t: &Tokens, state: &mut ProjectSettings, can_install: bool, focus_first: bool) -> Vec<Act> {
    let mut acts = Vec::new();
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        track_keyboard_focus(ui);
        top_bar(ui, t, state, &mut acts);
        ScrollArea::vertical().id_salt("project_settings_page").auto_shrink([false, false]).show(ui, |ui| {
            let avail = ui.available_width();
            let width = avail.min(MAX_WIDTH);
            let rect = Rect::from_min_size(pos2(ui.cursor().min.x + (avail - width) / 2.0, ui.cursor().min.y), vec2(width, ui.available_height()));
            ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                Frame::new().inner_margin(Margin { left: 40, right: 40, top: 36, bottom: 56 }).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 32.0);
                    ui.set_width(width - 80.0);
                    intro(ui, t, can_install);
                    add_card(ui, t, state, can_install, focus_first, &mut acts);
                    installed(ui, t, state, can_install, &mut acts);
                    if can_install {
                        themes(ui, t, state, &mut acts);
                    }
                });
            });
        });
    });
    acts
}

fn top_bar(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, acts: &mut Vec<Act>) {
    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAR_H), Sense::hover());
    ui.painter().rect_filled(bar, CornerRadius::ZERO, t.bg);
    ui.painter().hline(bar.x_range(), bar.max.y - 0.5, Stroke::new(1.0, t.border));
    let centre_y = bar.center().y;
    let name = ui.painter().text(pos2(bar.min.x + 20.0, centre_y), Align2::LEFT_CENTER, &state.project_name, regular(13.5), t.text_secondary);
    let slash = ui.painter().text(pos2(name.max.x + 6.0, centre_y), Align2::LEFT_CENTER, "/", regular(13.5), t.text_secondary);
    ui.painter().text(pos2(slash.max.x + 6.0, centre_y), Align2::LEFT_CENTER, tr("Source libraries"), regular(13.5), t.text);
    let inner = Rect::from_min_max(pos2(bar.min.x + 20.0, bar.min.y), pos2(bar.max.x - 20.0, bar.max.y));
    ui.scope_builder(UiBuilder::new().max_rect(inner).layout(Layout::right_to_left(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let (rect, _) = ui.allocate_exact_size(vec2(31.0, 32.0), Sense::hover());
        if button_at(ui, t, rect, tr("Close"), Look::Icon, true).clicked() {
            acts.push(Act::Close);
        }
        if button(ui, t, tr("Import components"), Look::Secondary, 34.0, 14.0, true).clicked() {
            acts.push(Act::Browse(None));
        }
    });
}

fn intro(ui: &mut Ui, t: &Tokens, can_install: bool) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.label(RichText::new(tr("Source libraries")).font(semibold(30.0)).color(t.text).line_height(Some(30.0 * 1.15)));
        let limit = text_width(ui, "0", regular(14.0)) * 70.0;
        ui.scope(|ui| {
            ui.set_max_width(limit.min(ui.available_width()));
            ui.add(
                Label::new(
                    RichText::new(tr(
                        "The libraries this project can import components from. They stay in the project and never go into what you export. Add one with its usual command: UIGEN detects its components.",
                    ))
                    .font(regular(14.0))
                    .color(t.text_secondary)
                    .line_height(Some(14.0 * BODY_LINE)),
                )
                .wrap(),
            );
        });
        if !can_install {
            super::note(ui, tr("Installing packages is only available in the desktop app"), t.text_secondary);
        }
    });
}

fn add_card(ui: &mut Ui, t: &Tokens, state: &mut ProjectSettings, can_install: bool, focus_first: bool, acts: &mut Vec<Act>) {
    card_frame(t, 20).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 14.0;
        heading(ui, t, tr("Add a library"));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let run_w = text_width(ui, tr("Run"), regular(13.5)) + 40.0;
            let field_w = (ui.available_width() - 10.0 - run_w).max(120.0);
            if command_field(ui, t, &mut state.package_input, field_w, focus_first) {
                acts.push(Act::Add);
            }
            if button(ui, t, tr("Run"), Look::Primary, 46.0, 20.0, can_install).clicked() {
                acts.push(Act::Add);
            }
        });
        match state.add_error() {
            Some(error) => {
                ui.add(Label::new(RichText::new(error).font(regular(12.5)).color(t.danger)).wrap());
            }
            None => examples(ui, t),
        }
        if let Some(run) = &state.last_run {
            log_block(ui, t, run);
            if let RunState::Done = run.state
                && let Some(row) = run.packages.last().and_then(|p| state.families.iter().find(|f| f.packages.contains(p) && f.status == "ready"))
            {
                ready_band(ui, t, row, can_install, acts);
            }
        }
    });
}

fn command_field(ui: &mut Ui, t: &Tokens, input: &mut String, width: f32, focus_first: bool) -> bool {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 46.0), Sense::hover());
    let radius = CornerRadius::same(12);
    ui.painter().rect_filled(rect, radius, t.field);
    ui.painter().rect_stroke(rect, radius, Stroke::new(1.0, t.border), StrokeKind::Inside);
    let dollar = ui.painter().text(pos2(rect.min.x + 14.0, rect.center().y), Align2::LEFT_CENTER, "$", mono(14.0), t.text_tertiary);
    let edit = Rect::from_min_max(pos2(dollar.max.x + 10.0, rect.min.y), pos2(rect.max.x - 14.0, rect.max.y));
    if focus_first {
        ui.memory_mut(|m| m.request_focus(package_field()));
    }
    let mut child = ui.new_child(UiBuilder::new().max_rect(edit).layout(Layout::left_to_right(Align::Center)));
    let response = child.add(
        TextEdit::singleline(input)
            .id(package_field())
            .frame(Frame::NONE)
            .font(mono(14.0))
            .align(Align2::LEFT_CENTER)
            .min_size(edit.size())
            .desired_width(edit.width()),
    );
    a11y::text(&response, tr("Install command"), input);
    note_focus(ui, package_field(), response.has_focus());
    if response.has_focus() {
        focus_ring(ui, t, rect, 12);
    }
    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

fn examples(ui: &mut Ui, t: &Tokens) {
    let format = |font: FontId| TextFormat { font_id: font, color: t.text_secondary, line_height: Some(EXAMPLES_LINE), ..TextFormat::default() };
    let mut job = LayoutJob::default();
    job.append(&format!("{} ", tr("Examples:")), 0.0, format(regular(12.5)));
    for (i, example) in EXAMPLES.iter().enumerate() {
        if i > 0 {
            job.append(" · ", 0.0, format(regular(12.5)));
        }
        job.append(example, 0.0, format(mono(12.5)));
    }
    ui.add(Label::new(job).wrap());
}

fn log_block(ui: &mut Ui, t: &Tokens, run: &super::LastRun) {
    Frame::new().fill(t.log).corner_radius(CornerRadius::same(12)).inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 4.0;
        let line = |ui: &mut Ui, text: &str, ink: Color32| {
            ui.add(Label::new(RichText::new(text).font(mono(12.5)).color(ink).line_height(Some(MONO_LINE))).wrap());
        };
        line(ui, &format!("$ {}", run.command), t.log_text);
        line(ui, tr("install scripts skipped"), t.log_muted);
        match &run.state {
            RunState::Running => line(ui, tr("Installing…"), t.log_muted),
            RunState::Done => {
                let message = trn("UIGEN: {n} libraries installed in this project", run.packages.len(), &[]);
                line(ui, &message, t.log_ok);
            }
            RunState::Failed(cause) => line(ui, &failure_text("", &run.packages.join(", "), cause), t.log_danger),
        }
    });
}

fn band_galley(ui: &Ui, text: String, font: FontId, color: Color32, wrap_width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::simple(text, font.clone(), color, wrap_width);
    for section in &mut job.sections {
        section.format.line_height = Some(font.size * BODY_LINE);
    }
    ui.painter().layout_job(job)
}

fn ready_band(ui: &mut Ui, t: &Tokens, row: &FamilyRow, can_install: bool, acts: &mut Vec<Act>) {
    const TICK: f32 = 36.0;
    const GAP: f32 = 14.0;
    const BUTTON_H: f32 = 34.0;
    const TEXT_MIN_H: f32 = 38.0;
    let mut facts = Vec::new();
    if let Some(framework) = framework_name(&row.framework) {
        facts.push(trf("Written for {framework}", &[("framework", &framework)]));
    }
    if !row.license.is_empty() {
        facts.push(trf("licence {licence}", &[("licence", &row.license)]));
    }
    let label = tr("Choose its components");
    let button_w = text_width(ui, label, regular(13.5)) + 28.0;
    let inner = ui.available_width() - 2.0 * ROW_PAD_X;
    let wrapped = inner < TICK + GAP + READY_TEXT_BASIS + GAP + button_w;
    let text_w = if wrapped { inner - TICK - GAP } else { inner - TICK - GAP - GAP - button_w };
    let title = band_galley(ui, trf("{package} is ready", &[("package", &row.name)]), semibold(13.5), t.text, text_w);
    let sub = band_galley(ui, facts.join(" · "), regular(12.5), t.text_secondary, text_w);
    let text_h = (title.size().y + sub.size().y).max(TEXT_MIN_H);
    let first_row = TICK.max(text_h);
    let height = 2.0 * GAP + if wrapped { first_row + GAP + BUTTON_H } else { first_row.max(BUTTON_H) };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(12), t.ok_bg);
    let top = rect.min.y + GAP;
    let tick = Rect::from_min_size(pos2(rect.min.x + ROW_PAD_X, top + (first_row - TICK) / 2.0), vec2(TICK, TICK));
    ui.painter().rect_filled(tick, CornerRadius::same(10), t.bg);
    let c = tick.center();
    let stroke = Stroke::new(2.5, t.ok_text);
    ui.painter().line_segment([c + vec2(-6.0, 0.0), c + vec2(-2.0, 4.5)], stroke);
    ui.painter().line_segment([c + vec2(-2.0, 4.5), c + vec2(6.0, -4.5)], stroke);
    let left = tick.max.x + GAP;
    let text_top = top + (first_row - title.size().y - sub.size().y) / 2.0;
    let sub_top = text_top + title.size().y;
    ui.painter().galley(pos2(left, text_top), title, t.text);
    ui.painter().galley(pos2(left, sub_top), sub, t.text_secondary);
    let button_rect = if wrapped {
        Rect::from_min_size(pos2(rect.min.x + ROW_PAD_X, top + first_row + GAP), vec2(button_w, BUTTON_H))
    } else {
        Rect::from_min_size(pos2(rect.max.x - ROW_PAD_X - button_w, rect.center().y - BUTTON_H / 2.0), vec2(button_w, BUTTON_H))
    };
    if button_at(ui, t, button_rect, label, Look::Primary, can_install).clicked() {
        acts.push(Act::Browse(Some(row.head().to_string())));
    }
}

fn framework_name(framework: &str) -> Option<&'static str> {
    match framework {
        "react" => Some("React"),
        "vue" => Some("Vue"),
        "svelte" => Some("Svelte"),
        "web-components" => Some("Web Components"),
        _ => None,
    }
}

fn installed(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, can_install: bool, acts: &mut Vec<Act>) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 14.0;
        heading(ui, t, tr("Installed in this project"));
        let orphans: Vec<String> = state
            .failures
            .iter()
            .filter(|f| !f.packages.iter().any(|p| state.sources.iter().any(|r| r.package == *p)))
            .filter(|f| !state.last_run.as_ref().is_some_and(|run| f.packages.iter().all(|p| run.packages.contains(p))))
            .map(|f| failure_text(&f.command, &f.packages.join(", "), &f.message))
            .collect();
        Frame::new().fill(t.bg).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(CARD_RADIUS)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            if state.sources.is_empty() && orphans.is_empty() {
                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 72.0), Sense::hover());
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    tr("No library yet. Run an install command above to add one."),
                    regular(13.5),
                    t.text_secondary,
                );
                return;
            }
            let width = ui.available_width();
            ScrollArea::horizontal().id_salt("project_sources_table").auto_shrink([false, true]).show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                table(ui, t, state, width, orphans.is_empty(), can_install, acts);
            });
            for message in &orphans {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(ROW_PAD_X);
                    ui.add(Label::new(RichText::new(message).font(regular(12.5)).color(t.danger)).wrap());
                });
                ui.add_space(12.0);
            }
        });
    });
}

fn state_tag(t: &Tokens, row: &FamilyRow) -> (&'static str, Color32, Color32) {
    match (row.status == "removed", row.status == "installing", row.work) {
        (true, ..) => (tr("Removed source"), t.hover, t.text_tertiary),
        (_, true, _) | (_, _, Work::Installing) => (tr("Installing…"), t.warn_bg, t.warn_text),
        (_, _, Work::Removing) => (tr("Removing…"), t.warn_bg, t.warn_text),
        _ => (tr("Ready"), t.ok_bg, t.ok_text),
    }
}

fn natural_widths(ui: &Ui, t: &Tokens, state: &ProjectSettings) -> [f32; COLUMNS] {
    let head = |title: &str| text_width(ui, title, regular(12.0));
    let mut width = [head(tr("Library")), head(tr("Version")), head(tr("Written for")), head(tr("State")), 0.0];
    for row in &state.families {
        width[0] = width[0].max(text_width(ui, &row.name, semibold(13.5)));
        width[1] = width[1].max(text_width(ui, &row.version, mono(13.5)));
        width[2] = width[2].max(framework_name(&row.framework).map_or(0.0, |name| tag_width(ui, name)));
        width[3] = width[3].max(tag_width(ui, state_tag(t, row).0));
    }
    width[4] = [tr("Browse"), tr("Change version"), tr("Remove")].iter().map(|label| text_width(ui, label, regular(13.5)) + 20.0).sum();
    width.map(|w| w + 2.0 * ROW_PAD_X)
}

fn column_edges(natural: &[f32; COLUMNS], width: f32) -> [f32; COLUMNS + 1] {
    let scale = width / natural.iter().sum::<f32>();
    let mut edges = [0.0; COLUMNS + 1];
    for (i, w) in natural.iter().enumerate() {
        edges[i + 1] = edges[i] + w * scale;
    }
    edges
}

fn table(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, width: f32, closes_table: bool, can_install: bool, acts: &mut Vec<Act>) {
    let natural = natural_widths(ui, t, state);
    let table_w = width.max(natural.iter().sum());
    let edges = column_edges(&natural, table_w);
    header(ui, t, table_w, &edges);
    let last = state.families.len().saturating_sub(1);
    for (i, row) in state.families.iter().enumerate() {
        table_row(ui, t, state, row, &Slot { width: table_w, edges: &edges, last: i == last && closes_table }, can_install, acts);
    }
}

fn header(ui: &mut Ui, t: &Tokens, width: f32, edges: &[f32; COLUMNS + 1]) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, HEAD_H), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, t.border));
    let y = rect.min.y + ROW_PAD_Y + 12.0 * BODY_LINE / 2.0;
    for (i, title) in [tr("Library"), tr("Version"), tr("Written for"), tr("State")].into_iter().enumerate() {
        ui.painter().text(pos2(rect.min.x + edges[i] + ROW_PAD_X, y), Align2::LEFT_CENTER, title, regular(12.0), t.text_secondary);
    }
}

struct Slot<'a> {
    width: f32,
    edges: &'a [f32; COLUMNS + 1],
    last: bool,
}

fn table_row(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, row: &FamilyRow, slot: &Slot, can_install: bool, acts: &mut Vec<Act>) {
    let Slot { width, edges, last } = *slot;
    let removed = row.status == "removed";
    let busy = row.status == "installing";
    let has_actions = !(removed || busy || row.work != Work::Idle);
    let note = row
        .packages
        .iter()
        .find_map(|p| state.failure(p))
        .or_else(|| state.changing.as_ref().filter(|(p, _)| p == row.head()).and_then(|_| state.change_error.clone()));
    let asked = (state.pending_removal() == Some(row.head())).then(|| state.removal_note(row.head()));
    let note_h = if note.is_some() || asked.is_some() { 24.0 } else { 0.0 };
    let content_h = if has_actions { ACTIONS_H } else { TAG_ROW_H };
    let height = 2.0 * ROW_PAD_Y + content_h + note_h + if last { 0.0 } else { 1.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    if !last {
        ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, t.border));
    }
    let y = rect.min.y + ROW_PAD_Y + content_h / 2.0;
    let ink = if removed { t.text_tertiary } else { t.text };
    ui.painter().text(pos2(rect.min.x + ROW_PAD_X, y), Align2::LEFT_CENTER, &row.name, semibold(13.5), ink);
    ui.painter().text(pos2(rect.min.x + edges[1] + ROW_PAD_X, y), Align2::LEFT_CENTER, &row.version, mono(13.5), ink);
    match framework_name(&row.framework) {
        Some(name) => tag_at(ui, rect.min.x + edges[2] + ROW_PAD_X, y + 0.5, name, t.hover, t.text_secondary),
        None => {
            ui.painter().text(pos2(rect.min.x + edges[2] + ROW_PAD_X, y), Align2::LEFT_CENTER, "—", regular(13.5), t.text_secondary);
        }
    }
    let (label, bg, fg) = state_tag(t, row);
    tag_at(ui, rect.min.x + edges[3] + ROW_PAD_X, y + 0.5, label, bg, fg);
    if let Some(message) = note {
        let at = pos2(rect.min.x + ROW_PAD_X, rect.max.y - ROW_PAD_Y - 6.0);
        ui.painter().text(at, Align2::LEFT_CENTER, message, regular(12.5), t.danger);
    } else if let Some(message) = asked {
        let at = pos2(rect.min.x + ROW_PAD_X, rect.max.y - ROW_PAD_Y - 6.0);
        ui.painter().text(at, Align2::LEFT_CENTER, message, regular(12.5), t.text_secondary);
    }
    if !has_actions {
        return;
    }
    let actions = Rect::from_min_max(
        pos2(rect.min.x + edges[4] + ROW_PAD_X, rect.min.y + ROW_PAD_Y),
        pos2(rect.min.x + edges[5] - ROW_PAD_X, rect.min.y + ROW_PAD_Y + ACTIONS_H),
    );
    let mut cell = ui.new_child(UiBuilder::new().max_rect(actions).layout(Layout::right_to_left(Align::Center)));
    row_actions(&mut cell, t, state, row, can_install, acts);
}

fn row_actions(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, row: &FamilyRow, can_install: bool, acts: &mut Vec<Act>) {
    let package = row.head().to_string();
    if state.pending_removal() == Some(row.head()) {
        ui.spacing_mut().item_spacing.x = 8.0;
        if crate::widgets::button(ui, tr("Cancel"), false, true).clicked() {
            acts.push(Act::CancelRemoval);
        }
        if crate::widgets::button(ui, tr("Confirm removal"), true, can_install).clicked() {
            acts.push(Act::ConfirmRemoval(package));
        }
    } else if let Some((_, draft)) = state.changing.as_ref().filter(|(p, _)| p == row.head()) {
        ui.spacing_mut().item_spacing.x = 8.0;
        if crate::widgets::button(ui, tr("Cancel"), false, true).clicked() {
            acts.push(Act::CancelChange);
        }
        if crate::widgets::button(ui, tr("Apply"), true, can_install).clicked() {
            acts.push(Act::Apply(package));
        }
        let field = egui::Id::unique(("project_settings_change", row.head()));
        if let Some(text) = crate::widgets::text_field(ui, field, draft, 90.0, tr("Version")) {
            acts.push(Act::Draft(text));
        }
    } else {
        ui.spacing_mut().item_spacing.x = 0.0;
        if button(ui, t, tr("Remove"), Look::Ghost, ACTIONS_H, 10.0, can_install).clicked() {
            acts.push(Act::AskRemoval(package.clone()));
        }
        if button(ui, t, tr("Change version"), Look::Ghost, ACTIONS_H, 10.0, can_install).clicked() {
            acts.push(Act::Change(package.clone()));
        }
        if button(ui, t, tr("Browse"), Look::Ghost, ACTIONS_H, 10.0, can_install).clicked() {
            acts.push(Act::Browse(Some(package)));
        }
    }
}

fn themes(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, acts: &mut Vec<Act>) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 14.0;
        heading(ui, t, tr("Token themes"));
        let mut found = Vec::new();
        project_themes::section(ui, t, &state.themes, &mut found);
        acts.extend(found.into_iter().map(Act::Themes));
    });
}
