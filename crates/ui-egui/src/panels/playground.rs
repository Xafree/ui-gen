//! The Playground tab: the page the capture service keeps open, shown as the images it sends, with
//! the pointer and keyboard relayed to it. The view takes the canvas area while the tab is in front.

use std::time::Duration;

use egui::{Align, Color32, Layout, Margin, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Ui, pos2, vec2};
use figcraft_engine::EngineError;
use figcraft_engine::playground::{NO_COMPONENT, Status};
use serde_json::json;

use super::playground_bars::{self, Bars, Snapshot};
use super::playground_decode::Decoder;
use super::playground_input::{Frame, Relay};
use crate::FigApp;
use crate::a11y;
use crate::i18n::{tr, trn};
use crate::theme::{Tokens, regular, semibold};
use crate::widgets;

const OPEN: &str = "playground.open";
const REFRESH: &str = "playground.refresh";
const CLOSE: &str = "playground.close";
const RESIZE: &str = "playground.resize";
const INPUT: &str = "playground.input";
const RESIZE_DELAY: f64 = 0.25;
const PAGE_WIDTH: (f32, f32) = (320.0, 3840.0);
const PAGE_HEIGHT: (f32, f32) = (240.0, 2160.0);
const POLL_EVERY: Duration = Duration::from_millis(33);
const BUSY_POLL_EVERY: Duration = Duration::from_millis(250);
const QUIET: f64 = 1.0;
const RESUME_AFTER: f64 = 0.5;
const INPUT_BOUND: usize = 256;
const MESSAGE_WIDTH: f32 = 420.0;
const MAX_LISTED: usize = 3;
const KEPT_SHORTCUTS: [&str; 8] =
    ["app.actions", "help.shortcuts", "view.fullscreen", "app.preferences", "file.new", "file.open", "file.save", "file.saveAs"];

/// The size of the page's window, as the service gets it: points of the view, the pixel ratio of the screen.
#[derive(Clone, Copy, PartialEq)]
struct Window {
    width: u32,
    height: u32,
    scale: f32,
}

#[derive(Default)]
struct Resizing {
    sent: Option<Window>,
    wanted: Option<(Window, f64)>,
}

/// When the view asks the page for images: while something just happened (an image, an input, a
/// setting, the tab coming back) and for a moment after it, and as long as something waits. A page
/// that stays the same is not asked again.
#[derive(Default)]
struct Pace {
    touched: bool,
    active_until: f64,
    polled: Option<f64>,
    seen: f64,
    signature: u64,
}

impl Pace {
    fn touch(&mut self) {
        self.touched = true;
    }

    fn arrive(&mut self, now: f64, signature: u64) {
        if self.touched || signature != self.signature || now - self.seen > RESUME_AFTER {
            self.extend(now);
        }
        self.touched = false;
        self.seen = now;
        self.signature = signature;
    }

    fn extend(&mut self, now: f64) {
        self.active_until = now + QUIET;
    }

    fn active(&self, now: f64) -> bool {
        now < self.active_until
    }

    fn poll_due(&mut self, now: f64) -> bool {
        let due = self.polled.is_none_or(|at| now - at >= POLL_EVERY.as_secs_f64());
        if due {
            self.polled = Some(now);
        }
        due
    }
}

/// The Playground tab of the window and what its view keeps between frames.
#[derive(Default)]
pub struct PlaygroundTab {
    pub open: bool,
    pub front: bool,
    pub empty: bool,
    pub doc_uid: u64,
    pub save_open: bool,
    pub code_open: bool,
    pub bars: Bars,
    picture: Option<TextureHandle>,
    decoder: Option<Decoder>,
    resizing: Resizing,
    relay: Relay,
    pace: Pace,
}

impl PlaygroundTab {
    fn start(&mut self, doc_uid: u64, empty: bool) {
        self.open = true;
        self.front = true;
        self.empty = empty;
        self.doc_uid = doc_uid;
        self.forget_page();
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn forget_page(&mut self) {
        self.picture = None;
        self.decoder = None;
        self.resizing = Resizing::default();
        self.relay = Relay::default();
        self.pace = Pace::default();
        self.pace.touch();
        self.bars.errors.clear();
    }
}

fn active_uid(app: &FigApp) -> u64 {
    app.session.docs.get(app.session.active).map_or(0, |d| d.uid)
}

/// The tab is in front and fills the canvas area.
pub fn on_screen(app: &FigApp) -> bool {
    let tab = &app.ui.playground;
    tab.open && tab.front && app.ui.present.is_none() && !app.ui.home && app.has_files()
}

/// Called after every command the interface runs: a `playground.open` that went through puts the
/// tab in front, one refused for want of a component opens it empty. Returns whether the refusal
/// is already told by the tab's empty state.
pub fn after_command(app: &mut FigApp, id: &str, result: &Result<serde_json::Value, EngineError>) -> bool {
    if id.starts_with("playground.") {
        app.ui.playground.pace.touch();
    }
    if id != OPEN && id != REFRESH {
        return false;
    }
    let uid = active_uid(app);
    match result {
        Ok(_) => {
            app.ui.playground.start(uid, false);
            if id == OPEN {
                app.ui.nav.region = a11y::Region::Toolbar;
            }
            false
        }
        Err(EngineError::BadParams { msg, .. }) if id == OPEN && msg == NO_COMPONENT && *app.session.playground_status() == Status::Closed => {
            app.ui.playground.start(uid, true);
            true
        }
        Err(_) => false,
    }
}

/// Keeps the tab as the engine's playground: it appears when one opens, goes when it closes, and
/// steps back when another file is in front.
pub fn sync(app: &mut FigApp) {
    let live = *app.session.playground_status() != Status::Closed;
    let uid = active_uid(app);
    let tab = &mut app.ui.playground;
    if live && (!tab.open || tab.empty) {
        tab.start(uid, false);
    } else if !live && tab.open && !tab.empty {
        tab.reset();
    }
    if tab.front && tab.doc_uid != uid {
        tab.front = false;
    }
}

/// Closes the tab: the page is told to close.
pub fn close(app: &mut FigApp) {
    app.run(CLOSE, json!({}));
    app.ui.playground.reset();
}

/// Brings the tab to the front, with its file.
pub fn show_tab(app: &mut FigApp) {
    let wanted = app.ui.playground.doc_uid;
    if let Some(index) = app.session.docs.iter().position(|d| d.uid == wanted && app.shows(d.uid))
        && index != app.session.active
    {
        app.run("doc.switch", json!({"index": index}));
    }
    app.ui.home = false;
    app.ui.playground.front = true;
    app.ui.playground.pace.touch();
}

/// Whether the shortcut of command `id` still works while the tab is in front: the document it
/// would act on is out of sight.
pub fn keeps_shortcut(id: &str) -> bool {
    KEPT_SHORTCUTS.contains(&id)
}

/// Draws the view in the canvas area `ui` covers, whatever state the playground is in.
pub fn show(app: &mut FigApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let status = app.session.playground_status().clone();
    let snap = if status == Status::Ready { playground_bars::snapshot(app) } else { Snapshot::default() };
    let bars = if status == Status::Closed { 0.0 } else { playground_bars::show(app, ui, &status, &snap) };
    let outer = ui.max_rect();
    let below_bars = Rect::from_min_max(pos2(outer.min.x, outer.min.y + bars), outer.max);
    let band = if status == Status::Ready { notices(app, ui, &t, below_bars, &snap) } else { 0.0 };
    let zone = Rect::from_min_max(pos2(below_bars.min.x, below_bars.min.y + band), outer.max);
    ui.scope_builder(egui::UiBuilder::new().max_rect(zone), |ui| match status {
        Status::Closed => empty(app, ui, &t),
        Status::Opening => {
            app.ui.playground.forget_page();
            loading(ui, &t);
        }
        Status::Failed(cause) => failure(app, ui, &t, &cause),
        Status::Ready => page(app, ui, &t, zone, &snap),
    });
}

fn centered(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let zone = ui.max_rect();
    let width = MESSAGE_WIDTH.min(zone.width() - 32.0).max(0.0);
    let column = Rect::from_center_size(zone.center(), vec2(width, zone.height()));
    ui.scope_builder(egui::UiBuilder::new().max_rect(column).layout(Layout::top_down(Align::Center)), |ui| {
        ui.add_space((zone.height() * 0.5 - 70.0).max(0.0));
        add(ui);
    });
}

fn title(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.label(RichText::new(text).font(semibold(14.0)).color(t.text));
    ui.add_space(6.0);
}

fn body(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).font(regular(12.0)).color(t.text_secondary)).wrap());
    ui.add_space(12.0);
}

fn empty(app: &mut FigApp, ui: &mut Ui, t: &Tokens) {
    let mut export = false;
    centered(ui, |ui| {
        title(ui, t, tr("Nothing to show in the playground yet"));
        body(ui, t, tr("Add components to the library, then export it as Lit: the playground shows that export."));
        export = link(ui, t, tr("Export library as Lit"));
    });
    if export {
        app.run("export.litWindow", json!({}));
    }
}

fn link(ui: &mut Ui, t: &Tokens, label: &str) -> bool {
    let galley = ui.painter().layout_no_wrap(label.to_string(), regular(12.0), t.accent_text);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(8.0, 6.0), Sense::click());
    a11y::link(&resp, label);
    let text_at = rect.min + vec2(4.0, 3.0);
    ui.painter().galley(text_at, galley.clone(), t.accent_text);
    if resp.hovered() || resp.has_focus() {
        let y = text_at.y + galley.size().y;
        ui.painter().line_segment([pos2(text_at.x, y), pos2(text_at.x + galley.size().x, y)], Stroke::new(1.0, t.accent_text));
    }
    a11y::ring_around(ui, rect, &resp);
    resp.clicked()
}

fn loading(ui: &mut Ui, t: &Tokens) {
    centered(ui, |ui| {
        ui.add(egui::Spinner::new().color(t.text_secondary));
        ui.add_space(8.0);
        body(ui, t, tr("Loading the playground…"));
    });
    ui.ctx().request_repaint_after(POLL_EVERY);
}

fn failure(app: &mut FigApp, ui: &mut Ui, t: &Tokens, cause: &str) {
    let cause = without_root(app, cause);
    let mut retry = false;
    centered(ui, |ui| {
        title(ui, t, tr("The playground couldn't open"));
        body(ui, t, &cause);
        retry = widgets::button(ui, tr("Try again"), true, true).clicked();
    });
    if retry {
        app.run(REFRESH, json!({}));
    }
}

fn without_root(app: &FigApp, message: &str) -> String {
    let Some(project) = app.session.project() else { return message.to_string() };
    let root = project.root.as_str();
    if root.is_empty() {
        return message.to_string();
    }
    message.replace(root, ".").replace(&root.replace('\\', "/"), ".")
}

fn page(app: &mut FigApp, ui: &mut Ui, t: &Tokens, zone: Rect, snap: &Snapshot) {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let busy = app.session.playground_busy();
    let tab = &mut app.ui.playground;
    tab.pace.arrive(now, snap.signature);
    let waiting = busy || snap.waiting() || tab.relay.waiting() || tab.picture.is_none();
    if (waiting || tab.pace.active(now)) && tab.pace.poll_due(now) {
        app.session.playground_poll();
    }
    if let Some(frame) = app.session.playground_take_frame() {
        tab.decoder.get_or_insert_with(|| Decoder::new(&ctx)).submit(frame);
        tab.pace.extend(now);
    }
    match tab.decoder.as_mut().and_then(Decoder::take) {
        Some(Some(picture)) => match tab.picture.as_mut() {
            Some(texture) => texture.set(picture, TextureOptions::LINEAR),
            None => tab.picture = Some(ctx.load_texture("playground-page", picture, TextureOptions::LINEAR)),
        },
        Some(None) => log::warn!("the playground sent an image that cannot be shown"),
        None => {}
    }
    if waiting {
        ctx.request_repaint_after(if busy { BUSY_POLL_EVERY } else { POLL_EVERY });
    } else if tab.pace.active(now) {
        ctx.request_repaint_after(POLL_EVERY);
    }
    let Some(texture) = tab.picture.clone() else {
        loading(ui, t);
        return;
    };
    let shown = page_rect(&ctx, zone);
    let resp = ui.interact(zone, ui.scope_id().with("playground_view"), Sense::click_and_drag());
    ui.painter_at(zone).image(texture.id(), shown, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    a11y::page_view(&mut app.ui.nav, &resp, tr("Playground preview"));
    a11y::ring_around(ui, zone.shrink(3.0), &resp);
    if resp.contains_pointer() && ui.input(|i| i.pointer.any_pressed()) {
        resp.request_focus();
    }
    relay(app, &ctx, &resp, shown, snap, now);
    resize(app, &ctx, zone);
}

fn page_rect(ctx: &egui::Context, zone: Rect) -> Rect {
    let window = window_of(ctx, zone);
    Rect::from_min_size(zone.min, vec2(window.width as f32, window.height as f32))
}

fn relay(app: &mut FigApp, ctx: &egui::Context, resp: &egui::Response, shown: Rect, snap: &Snapshot, now: f64) {
    let tab = &mut app.ui.playground;
    let escape_taken = tab.save_open || tab.code_open || egui::Popup::is_any_open(ctx);
    let (events, pointer) = ctx.input(|i| (i.events.clone(), i.pointer.latest_pos()));
    let frame = Frame { events: &events, pointer, zone: shown, hovered: resp.contains_pointer(), focused: resp.has_focus(), escape_taken };
    let relayed = tab.relay.translate(&frame);
    if let Some(step) = relayed.leave {
        resp.surrender_focus();
        app.ui.nav.step(step);
    }
    let relay = &mut app.ui.playground.relay;
    relay.queue(relayed.events);
    let batch = relay.take(INPUT_BOUND.saturating_sub(snap.input_waiting));
    if batch.is_empty() {
        return;
    }
    match app.session.execute(INPUT, &json!({"events": batch})) {
        Ok(_) => {
            let tab = &mut app.ui.playground;
            tab.relay.accepted();
            tab.pace.extend(now);
        }
        Err(error) => {
            let relay = &mut app.ui.playground.relay;
            if relay.first_refusal(&error.to_string()) {
                log::debug!("{INPUT}: {error}");
            }
            relay.put_back(batch);
        }
    }
}

fn window_of(ctx: &egui::Context, zone: Rect) -> Window {
    let fit = |extent: f32, (least, most): (f32, f32)| extent.round().clamp(least, most) as u32;
    Window {
        width: fit(zone.width(), PAGE_WIDTH),
        height: fit(zone.height(), PAGE_HEIGHT),
        scale: (ctx.pixels_per_point() * 100.0).round().clamp(100.0, 200.0) / 100.0,
    }
}

fn resize(app: &mut FigApp, ctx: &egui::Context, zone: Rect) {
    let now = ctx.input(|i| i.time);
    let wanted = window_of(ctx, zone);
    let resizing = &mut app.ui.playground.resizing;
    if resizing.sent == Some(wanted) {
        resizing.wanted = None;
        return;
    }
    let since = match resizing.wanted {
        Some((window, since)) if window == wanted => since,
        _ => now,
    };
    resizing.wanted = Some((wanted, since));
    let left = RESIZE_DELAY - (now - since);
    if left > 0.0 {
        ctx.request_repaint_after(Duration::from_secs_f64(left));
        return;
    }
    let params = json!({"width": wanted.width, "height": wanted.height, "scale": wanted.scale});
    match app.session.execute(RESIZE, &params) {
        Ok(_) => {
            app.ui.playground.resizing = Resizing { sent: Some(wanted), wanted: None };
            app.ui.playground.pace.touch();
        }
        Err(error) => log::debug!("{RESIZE}: {error}"),
    }
}

fn too_small(zone: Rect) -> bool {
    zone.width() < PAGE_WIDTH.0 || zone.height() < PAGE_HEIGHT.0
}

fn listing(ui: &mut Ui, t: &Tokens, title: &str, lines: &[String]) {
    ui.label(RichText::new(title).font(semibold(12.0)).color(t.text));
    for line in lines.iter().take(MAX_LISTED) {
        ui.add(egui::Label::new(RichText::new(line).font(regular(12.0)).color(t.text_secondary)).wrap());
    }
    if lines.len() > MAX_LISTED {
        ui.label(RichText::new(trn("and {n} more", lines.len() - MAX_LISTED, &[])).font(regular(12.0)).color(t.text_secondary));
    }
}

fn notices(app: &mut FigApp, ui: &mut Ui, t: &Tokens, zone: Rect, snap: &Snapshot) -> f32 {
    let (busy, stale) = (app.session.playground_busy(), app.session.playground_stale());
    let errors = app.ui.playground.bars.errors.clone();
    let missing = if snap.missing.is_empty() { Vec::new() } else { vec![snap.missing.join(", ")] };
    let notes: Vec<String> = snap
        .notes
        .iter()
        .map(|note| if note.component.is_empty() { note.message.clone() } else { format!("{}: {}", note.component, note.message) })
        .collect();
    if too_small(zone) || (!busy && !stale && errors.is_empty() && missing.is_empty() && notes.is_empty()) {
        return 0.0;
    }
    let mut refresh = false;
    let band = egui::Frame::NONE.fill(t.panel).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 6.0;
        if busy {
            ui.add(
                egui::Label::new(
                    RichText::new(tr("The capture service is busy: the page updates when it is free.")).font(regular(12.0)).color(t.text),
                )
                .wrap(),
            );
        }
        if stale {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(tr("The file changed since the page was exported.")).font(regular(12.0)).color(t.text));
                refresh = widgets::button(ui, tr("Refresh"), false, true).clicked();
            });
        }
        for (title, lines) in [
            (tr("Errors reported by the page"), &errors),
            (tr("Components missing from the library"), &missing),
            (tr("What the export could not do"), &notes),
        ] {
            if !lines.is_empty() {
                listing(ui, t, title, lines);
            }
        }
    });
    let rect = band.response.rect;
    ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, t.border));
    if refresh {
        app.run(REFRESH, json!({}));
    }
    rect.height()
}
