use egui::{
    Align, Align2, CornerRadius, FontId, Frame, Label, Layout, Margin, Rect, Response, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder,
    pos2, vec2,
};

use super::{Act, ProjectSettings, RunState, SourceRow, Work, failure_text, package_field, project_themes};
use crate::a11y;
use crate::i18n::{tr, trf, trn};
use crate::theme::{Tokens, regular, semibold};

const MAX_WIDTH: f32 = 1120.0;
const BAR_H: f32 = 52.0;
const CARD_RADIUS: u8 = 14;
const ROW_PAD_X: f32 = 16.0;
const ROW_PAD_Y: f32 = 12.0;
const ROW_CONTENT_H: f32 = 32.0;
const HEAD_H: f32 = 41.0;
const VERSION_W: f32 = 120.0;
const WRITTEN_W: f32 = 140.0;
const STATE_W: f32 = 150.0;
const LOG_MUTED: egui::Color32 = egui::Color32::from_rgb(0xA3, 0xA3, 0xA3);
const LOG_OK: egui::Color32 = egui::Color32::from_rgb(0x7B, 0xF1, 0xA8);
const EXAMPLES: [&str; 2] = ["npm install @heroui/react", "npm i @mui/material @emotion/react @emotion/styled"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Secondary,
    Primary,
    Ghost,
}

fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

fn text_width(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), font, egui::Color32::WHITE).size().x
}

fn button_at(ui: &mut Ui, t: &Tokens, rect: Rect, label: &str, look: Look, enabled: bool) -> Response {
    let resp = ui.interact(
        rect,
        ui.scope_id().with(("page_button", label, rect.min.x as i32, rect.min.y as i32)),
        if enabled { Sense::click() } else { Sense::hover() },
    );
    a11y::button(&resp, label);
    let hovered = enabled && resp.hovered();
    let radius = CornerRadius::same(if look == Look::Ghost { 8 } else { 9 });
    let (fill, ink) = match (look, enabled) {
        (Look::Primary, true) => (t.accent, t.accent_on),
        (Look::Primary, false) => (t.field, t.text_tertiary),
        (Look::Secondary, true) => (if hovered { t.hover } else { t.bg }, t.text),
        (Look::Secondary, false) => (t.bg, t.text_tertiary),
        (Look::Ghost, true) => (if hovered { t.hover } else { egui::Color32::TRANSPARENT }, t.text_secondary),
        (Look::Ghost, false) => (egui::Color32::TRANSPARENT, t.text_tertiary),
    };
    ui.painter().rect_filled(rect, radius, fill);
    if look == Look::Secondary {
        ui.painter().rect_stroke(rect, radius, Stroke::new(1.0, t.border), StrokeKind::Inside);
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, regular(13.5), ink);
    resp
}

fn button(ui: &mut Ui, t: &Tokens, label: &str, look: Look, height: f32, pad: f32, enabled: bool) -> Response {
    let width = text_width(ui, label, regular(13.5)) + 2.0 * pad;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    button_at(ui, t, rect, label, look, enabled)
}

fn tag_at(ui: &Ui, left: f32, centre_y: f32, text: &str, bg: egui::Color32, ink: egui::Color32) {
    let width = text_width(ui, text, regular(11.5)) + 16.0;
    let rect = Rect::from_min_size(pos2(left, centre_y - 11.0), vec2(width, 22.0));
    ui.painter().rect_filled(rect, CornerRadius::same(11), bg);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, regular(11.5), ink);
}

fn card_frame(t: &Tokens, padding: i8) -> Frame {
    Frame::new().fill(t.bg).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(CARD_RADIUS)).inner_margin(Margin::same(padding))
}

fn heading(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.label(RichText::new(text).font(semibold(16.0)).color(t.text));
}

pub(super) fn show(ui: &mut Ui, t: &Tokens, state: &mut ProjectSettings, can_install: bool, focus_first: bool) -> Vec<Act> {
    let mut acts = Vec::new();
    top_bar(ui, t, state, &mut acts);
    egui::ScrollArea::vertical().id_salt("project_settings_page").auto_shrink([false, false]).show(ui, |ui| {
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
    acts
}

fn top_bar(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, acts: &mut Vec<Act>) {
    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAR_H), Sense::hover());
    ui.painter().rect_filled(bar, CornerRadius::ZERO, t.bg);
    ui.painter().hline(bar.x_range(), bar.max.y - 0.5, Stroke::new(1.0, t.border));
    let name = &state.project_name;
    let crumb = pos2(bar.min.x + 20.0, bar.center().y);
    let first = ui.painter().text(crumb, Align2::LEFT_CENTER, format!("{name} /"), regular(13.5), t.text_secondary);
    ui.painter().text(pos2(first.max.x + 6.0, crumb.y), Align2::LEFT_CENTER, tr("Source libraries"), regular(13.5), t.text);
    let inner = Rect::from_min_max(pos2(bar.min.x + 20.0, bar.min.y), pos2(bar.max.x - 20.0, bar.max.y));
    ui.scope_builder(UiBuilder::new().max_rect(inner).layout(Layout::right_to_left(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let (rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
        if button_at(ui, t, rect, tr("Close"), Look::Ghost, true).clicked() {
            acts.push(Act::Close);
        }
        let stroke = Stroke::new(1.5, t.text_secondary);
        let c = rect.center();
        ui.painter().line_segment([c + vec2(-4.5, -4.5), c + vec2(4.5, 4.5)], stroke);
        ui.painter().line_segment([c + vec2(-4.5, 4.5), c + vec2(4.5, -4.5)], stroke);
        if button(ui, t, tr("Import components"), Look::Secondary, 34.0, 14.0, true).clicked() {
            acts.push(Act::Browse(None));
        }
    });
}

fn intro(ui: &mut Ui, t: &Tokens, can_install: bool) {
    ui.spacing_mut().item_spacing.y = 6.0;
    ui.label(RichText::new(tr("Source libraries")).font(semibold(30.0)).color(t.text));
    ui.scope(|ui| {
        ui.set_max_width(590.0);
        ui.add(Label::new(RichText::new(tr("The libraries this project can import components from. They stay in the project and never go into what you export. Add one with its usual command: UIGEN detects its components.")).font(regular(14.0)).color(t.text_secondary)).wrap());
    });
    if !can_install {
        super::note(ui, tr("Installing packages is only available in the desktop app"), t.text_secondary);
    }
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
                && let Some(row) = run.packages.last().and_then(|p| state.sources.iter().find(|r| r.package == *p && r.status == "ready"))
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
    let dollar = ui.painter().text(pos2(rect.min.x + 14.0, rect.center().y), Align2::LEFT_CENTER, "$", mono(14.0), t.text_tertiary);
    let edit = Rect::from_min_max(pos2(dollar.max.x + 10.0, rect.min.y), pos2(rect.max.x - 14.0, rect.max.y));
    if focus_first {
        ui.memory_mut(|m| m.request_focus(package_field()));
    }
    let mut child = ui.new_child(UiBuilder::new().max_rect(edit).layout(Layout::left_to_right(Align::Center)));
    let response = child.add(TextEdit::singleline(input).id(package_field()).frame(Frame::NONE).font(mono(14.0)).desired_width(edit.width()));
    a11y::text(&response, tr("Install command"), input);
    let edge = if response.has_focus() { t.accent_text } else { t.border };
    ui.painter().rect_stroke(rect, radius, Stroke::new(1.0, edge), StrokeKind::Inside);
    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

fn examples(ui: &mut Ui, t: &Tokens) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let muted = |text: &str| RichText::new(text.to_owned()).font(regular(12.5)).color(t.text_secondary);
        ui.label(muted(&format!("{} ", tr("Examples:"))));
        for (i, example) in EXAMPLES.iter().enumerate() {
            if i > 0 {
                ui.label(muted(" · "));
            }
            ui.label(RichText::new(*example).font(mono(12.5)).color(t.text_secondary));
        }
    });
}

fn log_block(ui: &mut Ui, t: &Tokens, run: &super::LastRun) {
    Frame::new().fill(t.log).corner_radius(CornerRadius::same(12)).inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 4.0;
        let line = |ui: &mut Ui, text: &str, ink: egui::Color32| {
            ui.add(Label::new(RichText::new(text).font(mono(12.5)).color(ink)).wrap());
        };
        line(ui, &format!("$ {}", run.command), egui::Color32::WHITE);
        line(ui, tr("install scripts skipped"), LOG_MUTED);
        match &run.state {
            RunState::Running => line(ui, tr("Installing…"), LOG_MUTED),
            RunState::Done => {
                let message = trn("UIGEN: {n} libraries installed in this project", run.packages.len(), &[]);
                line(ui, &message, LOG_OK);
            }
            RunState::Failed(cause) => line(ui, &failure_text("", &run.packages.join(", "), cause), Tokens::dark().danger),
        }
    });
}

fn ready_band(ui: &mut Ui, t: &Tokens, row: &SourceRow, can_install: bool, acts: &mut Vec<Act>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(12), t.ok_bg);
    let tick = Rect::from_min_size(pos2(rect.min.x + 16.0, rect.center().y - 18.0), vec2(36.0, 36.0));
    ui.painter().rect_filled(tick, CornerRadius::same(10), t.bg);
    let c = tick.center();
    let stroke = Stroke::new(2.5, t.ok_text);
    ui.painter().line_segment([c + vec2(-6.0, 0.0), c + vec2(-2.0, 4.5)], stroke);
    ui.painter().line_segment([c + vec2(-2.0, 4.5), c + vec2(6.0, -4.5)], stroke);
    let left = tick.max.x + 14.0;
    ui.painter().text(
        pos2(left, rect.center().y - 9.0),
        Align2::LEFT_CENTER,
        trf("{package} is ready", &[("package", &row.package)]),
        semibold(13.5),
        t.text,
    );
    let mut facts = Vec::new();
    if !row.framework.is_empty() {
        facts.push(trf("Written for {framework}", &[("framework", &framework_name(&row.framework))]));
    }
    if !row.license.is_empty() {
        facts.push(trf("licence {licence}", &[("licence", &row.license)]));
    }
    ui.painter().text(pos2(left, rect.center().y + 9.0), Align2::LEFT_CENTER, facts.join(" · "), regular(12.5), t.text_secondary);
    let label = tr("Choose its components");
    let width = text_width(ui, label, regular(13.5)) + 28.0;
    let button_rect = Rect::from_min_size(pos2(rect.max.x - 16.0 - width, rect.center().y - 17.0), vec2(width, 34.0));
    if button_at(ui, t, button_rect, label, Look::Primary, can_install).clicked() {
        acts.push(Act::Browse(Some(row.package.clone())));
    }
}

fn framework_name(framework: &str) -> String {
    let mut chars = framework.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn installed(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, can_install: bool, acts: &mut Vec<Act>) {
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
        header(ui, t);
        for (i, row) in state.sources.iter().enumerate() {
            table_row(ui, t, state, row, i > 0, can_install, acts);
        }
        for message in &orphans {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD_X);
                ui.add(Label::new(RichText::new(message).font(regular(12.5)).color(t.danger)).wrap());
            });
            ui.add_space(12.0);
        }
    });
}

fn columns(width: f32, actions: f32) -> [f32; 5] {
    let first = (width - VERSION_W - WRITTEN_W - STATE_W - actions).max(160.0);
    let version = ROW_PAD_X + first;
    [0.0, version, version + VERSION_W, version + VERSION_W + WRITTEN_W, version + VERSION_W + WRITTEN_W + STATE_W]
}

fn header(ui: &mut Ui, t: &Tokens) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEAD_H), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, t.border));
    let x = columns(rect.width(), 0.0);
    for (offset, title) in
        [(ROW_PAD_X, tr("Library")), (x[1] + ROW_PAD_X, tr("Version")), (x[2] + ROW_PAD_X, tr("Written for")), (x[3] + ROW_PAD_X, tr("State"))]
    {
        ui.painter().text(pos2(rect.min.x + offset, rect.center().y), Align2::LEFT_CENTER, title, regular(12.0), t.text_secondary);
    }
}

fn table_row(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, row: &SourceRow, divider: bool, can_install: bool, acts: &mut Vec<Act>) {
    let removed = row.status == "removed";
    let note =
        state.failure(&row.package).or_else(|| state.changing.as_ref().filter(|(p, _)| *p == row.package).and_then(|_| state.change_error.clone()));
    let note_h = if note.is_some() { 24.0 } else { 0.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 2.0 * ROW_PAD_Y + ROW_CONTENT_H + note_h), Sense::hover());
    if divider {
        ui.painter().hline(rect.x_range(), rect.min.y + 0.5, Stroke::new(1.0, t.border));
    }
    let y = rect.min.y + ROW_PAD_Y + ROW_CONTENT_H / 2.0;
    let x = columns(rect.width(), 0.0);
    let ink = if removed { t.text_tertiary } else { t.text };
    let busy = row.status == "installing";
    ui.painter().text(pos2(rect.min.x + ROW_PAD_X, y), Align2::LEFT_CENTER, &row.package, semibold(13.5), ink);
    if !busy {
        ui.painter().text(pos2(rect.min.x + x[1] + ROW_PAD_X, y), Align2::LEFT_CENTER, &row.version, mono(13.5), ink);
        if row.framework.is_empty() {
            ui.painter().text(pos2(rect.min.x + x[2] + ROW_PAD_X, y), Align2::LEFT_CENTER, "—", regular(13.5), t.text_secondary);
        } else {
            tag_at(ui, rect.min.x + x[2] + ROW_PAD_X, y, &framework_name(&row.framework), t.hover, t.text_secondary);
        }
    }
    let (label, bg, fg) = match (removed, busy, row.work) {
        (true, ..) => (tr("Removed source"), t.hover, t.text_tertiary),
        (_, true, _) | (_, _, Work::Installing) => (tr("Installing…"), t.warn_bg, t.warn_text),
        (_, _, Work::Removing) => (tr("Removing…"), t.warn_bg, t.warn_text),
        _ => (tr("Ready"), t.ok_bg, t.ok_text),
    };
    tag_at(ui, rect.min.x + x[3] + ROW_PAD_X, y, label, bg, fg);
    if let Some(message) = note {
        let at = pos2(rect.min.x + ROW_PAD_X, rect.max.y - ROW_PAD_Y - 6.0);
        ui.painter().text(at, Align2::LEFT_CENTER, message, regular(12.5), t.danger);
    }
    if removed || busy || row.work != Work::Idle {
        return;
    }
    let actions = Rect::from_min_max(
        pos2(rect.min.x + x[4] + ROW_PAD_X, rect.min.y + ROW_PAD_Y),
        pos2(rect.max.x - ROW_PAD_X, rect.min.y + ROW_PAD_Y + ROW_CONTENT_H),
    );
    ui.scope_builder(UiBuilder::new().max_rect(actions).layout(Layout::right_to_left(Align::Center)), |ui| {
        row_actions(ui, t, state, row, can_install, acts);
    });
}

fn row_actions(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, row: &SourceRow, can_install: bool, acts: &mut Vec<Act>) {
    ui.spacing_mut().item_spacing.x = 8.0;
    let package = row.package.clone();
    if state.pending_removal() == Some(row.package.as_str()) {
        if crate::widgets::button(ui, tr("Cancel"), false, true).clicked() {
            acts.push(Act::CancelRemoval);
        }
        if crate::widgets::button(ui, tr("Confirm removal"), true, can_install).clicked() {
            acts.push(Act::ConfirmRemoval(package));
        }
    } else if let Some((_, draft)) = state.changing.as_ref().filter(|(p, _)| *p == row.package) {
        if crate::widgets::button(ui, tr("Cancel"), false, true).clicked() {
            acts.push(Act::CancelChange);
        }
        if crate::widgets::button(ui, tr("Apply"), true, can_install).clicked() {
            acts.push(Act::Apply(package));
        }
        let field = egui::Id::unique(("project_settings_change", row.package.as_str()));
        if let Some(text) = crate::widgets::text_field(ui, field, draft, 90.0, tr("Version")) {
            acts.push(Act::Draft(text));
        }
    } else {
        if button(ui, t, tr("Remove"), Look::Ghost, 32.0, 10.0, can_install).clicked() {
            acts.push(Act::AskRemoval(package.clone()));
        }
        if button(ui, t, tr("Change version"), Look::Ghost, 32.0, 10.0, can_install).clicked() {
            acts.push(Act::Change(package.clone()));
        }
        if button(ui, t, tr("Browse"), Look::Ghost, 32.0, 10.0, can_install).clicked() {
            acts.push(Act::Browse(Some(package)));
        }
    }
}

fn themes(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, acts: &mut Vec<Act>) {
    ui.spacing_mut().item_spacing.y = 14.0;
    heading(ui, t, tr("Token themes"));
    let mut found = Vec::new();
    project_themes::section(ui, t, &state.themes, &mut found);
    acts.extend(found.into_iter().map(Act::Themes));
}
