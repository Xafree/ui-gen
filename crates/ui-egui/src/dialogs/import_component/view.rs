use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Align2, CornerRadius, Frame, Label, Layout, Margin, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2,
};

use crate::a11y;
use crate::dialogs::project_settings::page::{
    BAR_H, BODY_LINE, Look, MAX_WIDTH, button, note_focus, button_at, card_frame, focus_ring, keyboard_focus, mono, tag_at, tag_width, text_width,
    track_keyboard_focus,
};
use crate::i18n::{tr, trf, trn};
use crate::theme::{Tokens, regular, semibold};

pub(super) const STATES: [&str; 5] = ["Default", "Hover", "Focus", "Pressed", "Disabled"];
const NARROW: f32 = 420.0 + 28.0 + 280.0;
const GUTTER: f32 = 28.0;
const ASIDE_BASIS: f32 = 280.0;
const ASIDE_MAX: f32 = 340.0;
const HEAD_H: f32 = 43.6;
const ROW_H: f32 = 42.6;
const TAGGED_ROW_H: f32 = 45.0;
const CHIP_H: f32 = 28.0;
const CHIP_GAP: f32 = 6.0;
const BOX: f32 = 16.0;

pub(super) struct RowView {
    pub package: String,
    pub name: String,
    pub ticked: bool,
    pub taken_in: Option<String>,
}

pub(super) struct CardView {
    pub label: String,
    pub ticked: usize,
    pub total: usize,
    pub rows: Vec<RowView>,
}

pub(super) enum Status {
    Loading(String),
    Failed { package: String, cause: String },
}

pub(super) enum Progress {
    Idle,
    Importing { done: usize, total: usize },
    Stopping,
    Summary { done: usize, total: usize, failures: Vec<String> },
}

pub(super) enum Act {
    Toggle(String, String),
    State(usize),
    Theme(bool),
    Animations,
    Import,
    Cancel,
    Retry(String),
    OpenSettings,
    Show,
}

pub(super) struct Model<'a> {
    pub project: &'a str,
    pub cards: Vec<CardView>,
    pub statuses: Vec<Status>,
    pub no_source: bool,
    pub nothing_to_offer: Option<Vec<String>>,
    pub count: usize,
    pub states: [bool; 5],
    pub light: bool,
    pub dark: bool,
    pub animations: bool,
    pub search: &'a mut String,
    pub progress: Progress,
    pub message: Option<String>,
    pub at_top: bool,
    pub focus_first: bool,
}

fn state_label(state: &str) -> &'static str {
    match state {
        "Default" => tr("Default|state"),
        "Hover" => tr("Hover"),
        "Focus" => tr("Focus"),
        "Pressed" => tr("Pressed"),
        _ => tr("Disabled"),
    }
}

pub(super) fn show(ui: &mut Ui, t: &Tokens, model: &mut Model, acts: &mut Vec<Act>) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        track_keyboard_focus(ui);
        top_bar(ui, t, model.project, matches!(model.progress, Progress::Stopping), acts);
        let area = ScrollArea::vertical().id_salt("import_surface").auto_shrink([false, false]);
        let area = if model.at_top { area.vertical_scroll_offset(0.0) } else { area };
        area.show(ui, |ui| {
            let avail = ui.available_width();
            let width = avail.min(MAX_WIDTH);
            let rect = Rect::from_min_size(pos2(ui.cursor().min.x + (avail - width) / 2.0, ui.cursor().min.y), vec2(width, ui.available_height()));
            ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                Frame::new().inner_margin(Margin { left: 40, right: 40, top: 36, bottom: 56 }).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 24.0);
                    ui.set_width(width - 80.0);
                    steps(ui, t);
                    columns(ui, t, model, acts);
                });
            });
        });
    });
}

fn top_bar(ui: &mut Ui, t: &Tokens, project: &str, stopping: bool, acts: &mut Vec<Act>) {
    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAR_H), Sense::hover());
    ui.painter().rect_filled(bar, CornerRadius::ZERO, t.bg);
    ui.painter().hline(bar.x_range(), bar.max.y - 0.5, Stroke::new(1.0, t.border));
    let centre_y = bar.center().y;
    let name = ui.painter().text(pos2(bar.min.x + 20.0, centre_y), Align2::LEFT_CENTER, project, regular(13.5), t.text_secondary);
    let slash = ui.painter().text(pos2(name.max.x + 6.0, centre_y), Align2::LEFT_CENTER, "/", regular(13.5), t.text_secondary);
    let page = ui.painter().text(pos2(slash.max.x + 6.0, centre_y), Align2::LEFT_CENTER, tr("Import components"), regular(13.5), t.text);
    let crumb = ui.interact(name.union(page), ui.id().with("crumb"), Sense::hover());
    a11y::spoken(&crumb, &format!("{project} / {}", tr("Import components")));
    let inner = Rect::from_min_max(pos2(bar.min.x + 20.0, bar.min.y), pos2(bar.max.x - 20.0, bar.max.y));
    ui.scope_builder(UiBuilder::new().max_rect(inner).layout(Layout::right_to_left(Align::Center)), |ui| {
        let label = if stopping { tr("Stopping…") } else { tr("Cancel") };
        if button(ui, t, label, Look::Ghost, 32.0, 10.0, !stopping).clicked() {
            acts.push(Act::Cancel);
        }
    });
}

fn steps(ui: &mut Ui, t: &Tokens) {
    let gap = 8.0;
    let column = (ui.available_width() - gap) / 2.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 4.0 + 8.0 + 13.0 * BODY_LINE), Sense::hover());
    for (index, (number, label, weight)) in
        [("01", format!("{} ✓", tr("Libraries|import step")), false), ("02", tr("Components").to_string(), true)].into_iter().enumerate()
    {
        let left = rect.min.x + index as f32 * (column + gap);
        let bar = Rect::from_min_size(pos2(left, rect.min.y), vec2(column, 4.0));
        ui.painter().rect_filled(bar, CornerRadius::same(2), t.accent);
        let y = rect.min.y + 12.0 + 13.0 * BODY_LINE / 2.0;
        let digits = ui.painter().text(pos2(left, y), Align2::LEFT_CENTER, number, mono(13.0), t.text);
        let font = if weight { semibold(13.0) } else { regular(13.0) };
        let written = ui.painter().text(pos2(digits.max.x + 5.0, y), Align2::LEFT_CENTER, &label, font, t.text);
        let step = ui.interact(digits.union(written), ui.id().with(("step", index)), Sense::hover());
        a11y::spoken(&step, &format!("{number} {label}"));
    }
}

fn columns(ui: &mut Ui, t: &Tokens, model: &mut Model, acts: &mut Vec<Act>) {
    let inner = ui.available_width();
    if inner < NARROW {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 24.0;
            left(ui, t, model, acts);
            panel(ui, t, model, acts);
        });
        return;
    }
    let free = inner - 420.0 - GUTTER - ASIDE_BASIS;
    let aside = (ASIDE_BASIS + free / 1000.0).min(ASIDE_MAX);
    let main = inner - GUTTER - aside;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = GUTTER;
        ui.allocate_ui_with_layout(vec2(main, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(main);
            ui.spacing_mut().item_spacing.y = 18.0;
            left(ui, t, model, acts);
        });
        ui.allocate_ui_with_layout(vec2(aside, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(aside);
            ui.spacing_mut().item_spacing.y = 14.0;
            panel(ui, t, model, acts);
        });
    });
}

fn note(ui: &mut Ui, text: &str, size: f32, ink: egui::Color32) {
    ui.add(Label::new(RichText::new(text).font(regular(size)).color(ink).line_height(Some(size * BODY_LINE))).wrap());
}

fn left(ui: &mut Ui, t: &Tokens, model: &mut Model, acts: &mut Vec<Act>) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.label(RichText::new(tr("Choose your components")).font(semibold(30.0)).color(t.text).line_height(Some(30.0 * 1.15)));
        note(ui, tr("Take what you want from each library."), 14.0, t.text_secondary);
    });
    if model.no_source {
        note(ui, tr("No source library yet. Add one in the project settings."), 13.5, t.text_secondary);
        let open = button(ui, t, tr("Open project settings"), Look::Primary, 34.0, 14.0, true);
        if model.focus_first {
            open.request_focus();
        }
        if open.clicked() {
            acts.push(Act::OpenSettings);
        }
        return;
    }
    if !model.cards.is_empty() || model.search_active() {
        search_field(ui, model);
    }
    for status in &model.statuses {
        match status {
            Status::Loading(package) => note(ui, &trf("Reading the components of {package}…", &[("package", package)]), 13.5, t.text_secondary),
            Status::Failed { package, cause } => {
                let message = trf("Couldn't read the components of {package}: {cause}", &[("package", package), ("cause", cause)]);
                note(ui, &message, 13.5, t.danger);
                if button(ui, t, tr("Try again"), Look::Secondary, 34.0, 14.0, true).clicked() {
                    acts.push(Act::Retry(package.clone()));
                }
            }
        }
    }
    if let Some(notes) = &model.nothing_to_offer {
        note(ui, tr("This package exports no component."), 13.5, t.text_secondary);
        for line in notes {
            note(ui, line, 12.5, t.text_tertiary);
        }
    }
    if !model.cards.is_empty() {
        let query = model.search.trim().to_lowercase();
        let shown: Vec<&CardView> =
            model.cards.iter().filter(|card| query.is_empty() || card.rows.iter().any(|row| matches(&row.name, &query))).collect();
        if shown.is_empty() {
            note(ui, tr("No matching components"), 13.5, t.text_secondary);
        }
        for card in shown {
            library(ui, t, card, &query, acts);
        }
    }
}

fn matches(name: &str, query: &str) -> bool {
    name.to_lowercase().contains(query)
}

impl Model<'_> {
    fn search_active(&self) -> bool {
        !self.search.trim().is_empty()
    }
}

pub(super) fn search_id() -> egui::Id {
    egui::Id::unique("import_surface_search")
}

fn search_field(ui: &mut Ui, model: &mut Model) {
    let width = ui.available_width().min(320.0);
    let response = crate::widgets::text_input(ui, search_id(), model.search, width, tr("Search components"));
    a11y::text(&response, tr("Search components"), model.search);
    if model.focus_first {
        response.request_focus();
    }
    note_focus(ui, search_id(), response.has_focus());
}

fn library(ui: &mut Ui, t: &Tokens, card: &CardView, query: &str, acts: &mut Vec<Act>) {
    card_frame(t, 0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let (head, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEAD_H), Sense::hover());
        let top = CornerRadius { nw: 13, ne: 13, sw: 0, se: 0 };
        ui.painter().rect_filled(head, top, t.hover);
        let heading = ui.interact(head, ui.id().with(("card", card.label.as_str())), Sense::hover());
        a11y::spoken(&heading, &card.label);
        ui.painter().text(pos2(head.min.x + 16.0, head.center().y), Align2::LEFT_CENTER, &card.label, semibold(13.5), t.text);
        let count = trf("{n} of {m}", &[("n", &card.ticked), ("m", &card.total)]);
        ui.painter().text(pos2(head.max.x - 16.0, head.center().y), Align2::RIGHT_CENTER, count, regular(12.5), t.text_secondary);
        for row in card.rows.iter().filter(|row| query.is_empty() || matches(&row.name, query)) {
            component_row(ui, t, row, acts);
        }
    });
}

fn component_row(ui: &mut Ui, t: &Tokens, row: &RowView, acts: &mut Vec<Act>) {
    let height = if row.taken_in.is_some() { TAGGED_ROW_H } else { ROW_H };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let id = ui.scope_id().with(("component", row.package.as_str(), row.name.as_str()));
    let free = row.taken_in.is_none() || row.ticked;
    let resp = ui.interact(rect, id, if free { Sense::click() } else { Sense::hover() });
    a11y::checkbox(&resp, &row.name, row.ticked);
    if let Some(package) = &row.taken_in {
        a11y::describe(&resp, &trf("Already taken in {package}", &[("package", package)]));
    }
    if !free {
        a11y::disabled(&resp);
    }
    if resp.gained_focus() {
        resp.scroll_to_me(None);
    }
    if !ui.is_rect_visible(rect) {
        return;
    }
    ui.painter().hline(rect.x_range(), rect.min.y + 0.5, Stroke::new(1.0, t.border));
    if free && resp.hovered() {
        ui.painter().rect_filled(Rect::from_min_max(pos2(rect.min.x, rect.min.y + 1.0), rect.max), CornerRadius::ZERO, t.hover);
    }
    let centre_y = rect.min.y + 0.5 + (height - 1.0) / 2.0;
    let boxed = Rect::from_center_size(pos2(rect.min.x + 16.0 + BOX / 2.0, centre_y), vec2(BOX, BOX));
    draw_box(ui, t, boxed, row.ticked, free);
    let ink = if free { t.text } else { t.text_secondary };
    let name = ui.painter().text(pos2(boxed.max.x + 12.0, centre_y), Align2::LEFT_CENTER, &row.name, regular(13.5), ink);
    if let Some(package) = &row.taken_in {
        let text = trf("Already taken in {package}", &[("package", package)]);
        let left = (name.max.x + 12.0).max(rect.max.x - 16.0 - tag_width(ui, &text));
        tag_at(ui, left, centre_y, &text, t.warn_bg, t.warn_text);
    }
    if resp.has_focus() && keyboard_focus(ui) {
        focus_ring(ui, t, rect, 0);
    }
    if resp.clicked() {
        acts.push(Act::Toggle(row.package.clone(), row.name.clone()));
    }
}

fn draw_box(ui: &Ui, t: &Tokens, rect: Rect, on: bool, free: bool) {
    let painter = ui.painter();
    if on {
        painter.rect_filled(rect, CornerRadius::same(3), t.accent);
        let c = rect.center();
        let stroke = Stroke::new(2.0, t.accent_on);
        painter.line_segment([c + vec2(-4.0, 0.0), c + vec2(-1.0, 3.0)], stroke);
        painter.line_segment([c + vec2(-1.0, 3.0), c + vec2(4.5, -3.5)], stroke);
    } else {
        let (fill, ink) = if free { (t.box_off, t.box_off_border) } else { (egui::Color32::TRANSPARENT, t.text_tertiary) };
        painter.rect_filled(rect, CornerRadius::same(3), fill);
        painter.rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.0, ink), StrokeKind::Inside);
    }
}

fn panel(ui: &mut Ui, t: &Tokens, model: &mut Model, acts: &mut Vec<Act>) {
    card_frame(t, 20).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 14.0;
        note(ui, tr("Your selection"), 13.5, t.text_secondary);
        count_line(ui, t, model.count);
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, t.border));
        group(ui, t, tr("States"), |ui| {
            let labels: Vec<(&str, bool)> = STATES.iter().zip(model.states).map(|(state, on)| (state_label(state), on)).collect();
            if let Some(index) = chips(ui, t, &labels) {
                acts.push(Act::State(index));
            }
        });
        group(ui, t, tr("Themes"), |ui| {
            if let Some(index) = chips(ui, t, &[(tr("Light"), model.light), (tr("Dark"), model.dark)]) {
                acts.push(Act::Theme(index == 1));
            }
        });
        if chips(ui, t, &[(tr("Animations"), model.animations)]).is_some() {
            acts.push(Act::Animations);
        }
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
        if button_at(ui, t, rect, tr("Import"), Look::Primary, true).clicked() {
            acts.push(Act::Import);
        }
        status(ui, t, model, acts);
    });
}

fn group(ui: &mut Ui, t: &Tokens, title: &str, add: impl FnOnce(&mut Ui)) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        note(ui, title, 12.0, t.text_secondary);
        add(ui);
    });
}

fn count_line(ui: &mut Ui, t: &Tokens, count: usize) {
    let words = trn("{n} components", count, &[]);
    let rest = words.replacen(&count.to_string(), "", 1);
    let mut job = LayoutJob::default();
    job.append(&count.to_string(), 0.0, TextFormat { font_id: semibold(32.0), color: t.text, line_height: Some(46.0), ..TextFormat::default() });
    job.append(&rest, 0.0, TextFormat { font_id: regular(16.0), color: t.text_secondary, line_height: Some(46.0), ..TextFormat::default() });
    ui.add(Label::new(job));
}

fn chips(ui: &mut Ui, t: &Tokens, items: &[(&str, bool)]) -> Option<usize> {
    let width = ui.available_width();
    let widths: Vec<f32> = items.iter().map(|(name, _)| text_width(ui, name, regular(12.5)) + 22.0).collect();
    let mut rows = 1;
    let mut x = 0.0;
    for w in &widths {
        if x > 0.0 && x + w > width {
            rows += 1;
            x = 0.0;
        }
        x += w + CHIP_GAP;
    }
    let (area, _) = ui.allocate_exact_size(vec2(width, rows as f32 * CHIP_H + (rows - 1) as f32 * CHIP_GAP), Sense::hover());
    let (mut x, mut y) = (0.0, 0.0);
    let mut clicked = None;
    for (index, ((name, on), w)) in items.iter().zip(&widths).enumerate() {
        if x > 0.0 && x + w > width {
            x = 0.0;
            y += CHIP_H + CHIP_GAP;
        }
        let rect = Rect::from_min_size(pos2(area.min.x + x, area.min.y + y), vec2(*w, CHIP_H));
        let resp = ui.interact(rect, ui.scope_id().with(("chip", *name)), Sense::click());
        a11y::toggle(&resp, name, *on);
        let (fill, border, ink) =
            if *on { (t.selected_row, t.selected_row, t.accent_text) } else { (egui::Color32::TRANSPARENT, t.border, t.text_secondary) };
        ui.painter().rect_filled(rect, CornerRadius::same(14), fill);
        ui.painter().rect_stroke(rect, CornerRadius::same(14), Stroke::new(1.0, border), StrokeKind::Inside);
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, name, regular(12.5), ink);
        if resp.has_focus() && keyboard_focus(ui) {
            focus_ring(ui, t, rect, 14);
        }
        if resp.clicked() {
            clicked = Some(index);
        }
        x += w + CHIP_GAP;
    }
    clicked
}

fn status(ui: &mut Ui, t: &Tokens, model: &Model, acts: &mut Vec<Act>) {
    if let Some(message) = &model.message {
        note(ui, message, 12.5, t.danger);
    }
    match &model.progress {
        Progress::Idle => {}
        Progress::Importing { done, total } => note(ui, &trf("Importing… {done} of {total}", &[("done", done), ("total", total)]), 13.5, t.text),
        Progress::Stopping => note(ui, tr("Stopping… the component in progress finishes first"), 13.5, t.text),
        Progress::Summary { done, total, failures } => {
            note(ui, &trf("Imported {done} of {total} components", &[("done", done), ("total", total)]), 13.5, t.text);
            for failure in failures {
                note(ui, failure, 12.5, t.danger);
            }
            if *done > 0 && button(ui, t, tr("Show in Components"), Look::Secondary, 34.0, 14.0, true).clicked() {
                acts.push(Act::Show);
            }
        }
    }
}
