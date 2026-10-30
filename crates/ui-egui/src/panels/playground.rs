//! The Playground tab: the page the capture service keeps open, shown as the images it sends, with
//! the pointer and keyboard relayed to it. The view takes the canvas area while the tab is in front.

use std::io::Cursor;
use std::time::Duration;

use egui::{
    Align, Align2, Color32, CornerRadius, Id, Layout, Margin, Order, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Ui, pos2, vec2,
};
use figcraft_engine::EngineError;
use figcraft_engine::playground::{NO_COMPONENT, Status};
use serde_json::json;

use super::playground_bars::{self, Bars};
use super::playground_input::{Frame, Relay};
use crate::FigApp;
use crate::a11y;
use crate::i18n::tr;
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
const MAX_PICTURE_SIDE: u32 = 8192;
const MAX_PICTURE_BYTES: u64 = 256 << 20;
const POLL_EVERY: Duration = Duration::from_millis(33);
const BUSY_POLL_EVERY: Duration = Duration::from_millis(250);
const MESSAGE_WIDTH: f32 = 420.0;

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
    resizing: Resizing,
    relay: Relay,
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
        self.resizing = Resizing::default();
        self.relay = Relay::default();
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
}

/// Draws the view in the canvas area `ui` covers, whatever state the playground is in.
pub fn show(app: &mut FigApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let status = app.session.playground_status().clone();
    let bars = if status == Status::Closed { 0.0 } else { playground_bars::show(app, ui, &status) };
    let outer = ui.max_rect();
    let zone = Rect::from_min_max(pos2(outer.min.x, outer.min.y + bars), outer.max);
    ui.scope_builder(egui::UiBuilder::new().max_rect(zone), |ui| match status {
        Status::Closed => empty(app, ui, &t),
        Status::Opening => {
            app.ui.playground.forget_page();
            loading(ui, &t);
        }
        Status::Failed(cause) => failure(app, ui, &t, &cause),
        Status::Ready => page(app, ui, &t, zone),
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

fn page(app: &mut FigApp, ui: &mut Ui, t: &Tokens, zone: Rect) {
    let ctx = ui.ctx().clone();
    let busy = app.session.playground_busy();
    app.session.playground_poll();
    if let Some(frame) = app.session.playground_take_frame() {
        match decode(&frame) {
            Some(picture) => match app.ui.playground.picture.as_mut() {
                Some(texture) => texture.set(picture, TextureOptions::LINEAR),
                None => app.ui.playground.picture = Some(ctx.load_texture("playground-page", picture, TextureOptions::LINEAR)),
            },
            None => log::warn!("the playground sent an image that cannot be shown"),
        }
    }
    ctx.request_repaint_after(if busy { BUSY_POLL_EVERY } else { POLL_EVERY });
    let Some(texture) = app.ui.playground.picture.clone() else {
        loading(ui, t);
        notices(app, &ctx, t, zone);
        return;
    };
    let resp = ui.interact(zone, ui.scope_id().with("playground_view"), Sense::click_and_drag());
    ui.painter().image(texture.id(), zone, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    a11y::page_view(&mut app.ui.nav, &resp, tr("Playground preview"));
    a11y::ring_around(ui, zone.shrink(3.0), &resp);
    if resp.contains_pointer() && ui.input(|i| i.pointer.any_pressed()) {
        resp.request_focus();
    }
    relay(app, &ctx, &resp, zone);
    resize(app, &ctx, zone);
    notices(app, &ctx, t, zone);
}

fn relay(app: &mut FigApp, ctx: &egui::Context, resp: &egui::Response, zone: Rect) {
    let (events, pointer) = ctx.input(|i| (i.events.clone(), i.pointer.latest_pos()));
    let focused = resp.has_focus();
    let frame = Frame { events: &events, pointer, zone, hovered: resp.contains_pointer(), focused };
    let relayed = app.ui.playground.relay.translate(&frame);
    if let Some(step) = relayed.leave {
        resp.surrender_focus();
        app.ui.nav.step(step);
    }
    if !relayed.events.is_empty()
        && let Err(error) = app.session.execute(INPUT, &json!({"events": relayed.events}))
    {
        log::debug!("{INPUT}: {error}");
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
        Ok(_) => app.ui.playground.resizing = Resizing { sent: Some(wanted), wanted: None },
        Err(error) => log::debug!("{RESIZE}: {error}"),
    }
}

fn notices(app: &mut FigApp, ctx: &egui::Context, t: &Tokens, zone: Rect) {
    let (busy, stale) = (app.session.playground_busy(), app.session.playground_stale());
    if !busy && !stale {
        return;
    }
    let mut refresh = false;
    egui::Area::new(Id::unique("playground_notices"))
        .order(Order::Foreground)
        .pivot(Align2::CENTER_TOP)
        .fixed_pos(pos2(zone.center().x, zone.min.y + 12.0))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(t.panel)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    if busy {
                        ui.label(
                            RichText::new(tr("The capture service is busy: the page updates when it is free.")).font(regular(12.0)).color(t.text),
                        );
                    }
                    if stale {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("The file changed since the page was exported.")).font(regular(12.0)).color(t.text));
                            refresh = widgets::button(ui, tr("Refresh"), false, true).clicked();
                        });
                    }
                });
        });
    if refresh {
        app.run(REFRESH, json!({}));
    }
}

fn decode(frame: &figcraft_engine::playground::Frame) -> Option<egui::ColorImage> {
    use figcraft_engine::playground::FrameFormat;
    let format = match frame.format {
        FrameFormat::Jpeg => image::ImageFormat::Jpeg,
        FrameFormat::Png => image::ImageFormat::Png,
    };
    let mut reader = image::ImageReader::with_format(Cursor::new(&frame.data), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_PICTURE_SIDE);
    limits.max_image_height = Some(MAX_PICTURE_SIDE);
    limits.max_alloc = Some(MAX_PICTURE_BYTES);
    reader.limits(limits);
    let rgba = reader.decode().ok()?.into_rgba8();
    let size = [usize::try_from(rgba.width()).ok()?, usize::try_from(rgba.height()).ok()?];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}
