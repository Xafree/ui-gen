//! The detail window of a Compare row: the original and the library version of one state side by
//! side, one over the other behind a slider, or as the pixels that differ. Escape closes it. The
//! difference is drawn once per measured row (`Row::difference`).

use egui::{Align2, Context, CornerRadius, Id, LayerId, Order, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use figcraft_doc::NodeId;
use figcraft_engine::uigen_compare;

use crate::FigApp;
use crate::a11y;
use crate::compare_tab::{Pictures, Row, dimensions, original_raster};
use crate::dev_compare::{BAR_H, Place, Shot, message, pane};
use crate::i18n::{tr, trf};
use crate::icons::Icon;
use crate::theme::{ACCENT, Tokens, regular, semibold};
use crate::widgets;

const WINDOW: &str = "compare_detail";
const INITIAL_SPLIT: f32 = 0.5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum Mode {
    #[default]
    SideBySide,
    Overlay,
    Difference,
}

impl Mode {
    const ALL: [Mode; 3] = [Mode::SideBySide, Mode::Overlay, Mode::Difference];

    fn label(self) -> &'static str {
        match self {
            Self::SideBySide => tr("Side by side"),
            Self::Overlay => tr("Overlay"),
            Self::Difference => tr("Difference"),
        }
    }
}

pub(crate) struct View {
    opened: Option<NodeId>,
    mode: Mode,
    split: f32,
    focus: bool,
}

impl Default for View {
    fn default() -> Self {
        Self { opened: None, mode: Mode::default(), split: INITIAL_SPLIT, focus: false }
    }
}

impl View {
    fn opening(variant: NodeId) -> Self {
        Self { opened: Some(variant), focus: true, ..Self::default() }
    }
}

/// Escape closes the window (before the canvas's shortcuts see it).
pub fn keys(app: &mut FigApp, ctx: &Context) {
    let shown = app.ui.compare.detail.is_some_and(|variant| app.ui.compare.shows(variant));
    if shown && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        app.ui.compare.detail = None;
    }
}

/// The detail window of the row opened in the Compare tab, while open.
pub fn show(app: &mut FigApp, ctx: &Context) {
    let Some(variant) = app.ui.compare.detail else {
        app.ui.compare.view = View::default();
        return;
    };
    let mut view = std::mem::take(&mut app.ui.compare.view);
    let open = window(app, ctx, variant, &mut view);
    app.ui.compare.view = view;
    if !open {
        app.ui.compare.detail = None;
    }
}

fn window(app: &FigApp, ctx: &Context, variant: NodeId, view: &mut View) -> bool {
    let tab = &app.ui.compare;
    let Some(run) = tab.run() else { return false };
    let Some(row) = run.visible().iter().find(|row| row.variant == variant) else { return run.pending(variant) };
    let Some(pictures) = row.pictures.as_ref() else { return false };
    if view.opened != Some(variant) {
        *view = View::opening(variant);
    }
    let difference = (view.mode == Mode::Difference).then(|| row.difference.get_or_init(|| difference_of(app, ctx, variant, tab.dark())).as_ref());
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    let size = vec2(960.0_f32.min(screen.width() - 64.0).max(480.0), 600.0_f32.min(screen.height() - 96.0).max(320.0));
    let id = Id::unique(WINDOW);
    ctx.memory_mut(|m| m.set_modal_layer(LayerId::new(Order::Middle, id)));
    let mut open = true;
    egui::Window::new(row.title())
        .id(id)
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .fixed_size(size)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(screen.center())
        .frame(egui::Frame::window(&ctx.global_style()).fill(t.panel).inner_margin(egui::Margin::same(0)))
        .show(ctx, |ui| {
            ui.set_min_size(size);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            header(ui, &t, row, view, &mut open);
            let (body, _) = ui.allocate_exact_size(vec2(size.x, size.y - BAR_H * 2.0 - 2.0), Sense::hover());
            let scene = Scene { row, pictures, difference };
            scene.draw(ui, &t, body, view);
            footer(ui, &t, row, view);
        });
    open
}

fn difference_of(app: &FigApp, ctx: &Context, variant: NodeId, dark: bool) -> Option<Shot> {
    let original = original_raster(app, variant, dark)?;
    let library = uigen_compare::library_raster(&app.session.ds().ok()?.doc, variant, dark).ok()?;
    let (original, library) = uigen_compare::aligned(&original, &library);
    Shot::from_raster(ctx, &uigen_compare::difference_image(&original, &library))
}

fn header(ui: &mut Ui, t: &Tokens, row: &Row, view: &mut View, open: &mut bool) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAR_H), Sense::hover());
    let title = ui.painter().layout_no_wrap(row.title().to_owned(), semibold(13.0), t.text);
    let title_w = title.size().x;
    ui.painter().galley(pos2(r.min.x + 16.0, r.center().y - title.size().y / 2.0), title, t.text);
    if !row.axes.is_empty() {
        let axes = widgets::elide(ui, &row.axes, 240.0);
        ui.painter().text(pos2(r.min.x + 28.0 + title_w, r.center().y), Align2::LEFT_CENTER, axes, regular(12.0), t.text_secondary);
    }
    let galleys = Mode::ALL.map(|mode| ui.painter().layout_no_wrap(mode.label().into(), regular(11.0), t.text));
    let widths: f32 = galleys.iter().map(|g| g.size().x + 24.0).sum();
    let mut x = r.max.x - 48.0 - widths + 4.0;
    for (mode, galley) in Mode::ALL.into_iter().zip(galleys) {
        let cell = Rect::from_min_max(pos2(x, r.center().y - 12.0), pos2(x + galley.size().x + 20.0, r.center().y + 12.0));
        let resp = ui.interact(cell, ui.scope_id().with(mode), Sense::click());
        let on = view.mode == mode;
        a11y::tab(&resp, mode.label(), on);
        if on && std::mem::take(&mut view.focus) {
            resp.request_focus();
        }
        if on {
            ui.painter().rect_filled(cell, CornerRadius::same(5), t.field);
            ui.painter().rect_stroke(cell, CornerRadius::same(5), Stroke::new(1.0, t.border), StrokeKind::Inside);
        } else if resp.hovered() {
            ui.painter().rect_filled(cell, CornerRadius::same(5), t.hover);
        }
        ui.painter().galley(cell.center() - galley.size() / 2.0, galley, if on { t.text } else { t.text_secondary });
        if resp.clicked() {
            view.mode = mode;
        }
        x = cell.max.x + 4.0;
    }
    let close = Rect::from_center_size(pos2(r.max.x - 24.0, r.center().y), vec2(24.0, 24.0));
    if widgets::icon_button_at(ui, ui.scope_id().with("close"), close, Icon::Close, tr("Close")) {
        *open = false;
    }
    ui.painter().rect_filled(Rect::from_min_max(pos2(r.min.x, r.max.y - 1.0), r.max), CornerRadius::ZERO, t.border);
}

fn footer(ui: &mut Ui, t: &Tokens, row: &Row, view: &mut View) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAR_H + 1.0), Sense::hover());
    ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 1.0)), CornerRadius::ZERO, t.border);
    let inner = Rect::from_min_max(pos2(r.min.x + 16.0, r.min.y + 1.0), pos2(r.max.x - 16.0, r.max.y));
    match view.mode {
        Mode::Overlay => {
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
                ui.spacing_mut().slider_width = inner.width().min(420.0);
                let resp = ui.add(egui::Slider::new(&mut view.split, 0.0..=1.0).show_value(false));
                a11y::slider(&resp, tr("Comparison slider"), f64::from(view.split), true);
            });
        }
        Mode::SideBySide if !uigen_compare::sizes_agree(row.original, row.library) => {
            let note = trf(
                "Size differs: original {original}, library {library}",
                &[("original", &dimensions(row.original)), ("library", &dimensions(row.library))],
            );
            ui.painter().text(pos2(inner.min.x, inner.center().y), Align2::LEFT_CENTER, note, regular(11.0), t.text_secondary);
        }
        Mode::SideBySide | Mode::Difference => {}
    }
}

struct Scene<'a> {
    row: &'a Row,
    pictures: &'a Pictures,
    difference: Option<Option<&'a Shot>>,
}

impl Scene<'_> {
    fn draw(&self, ui: &mut Ui, t: &Tokens, r: Rect, view: &View) {
        let painter = ui.painter_at(r);
        painter.rect_filled(r, CornerRadius::ZERO, t.canvas);
        let labels_y = r.min.y + 28.0;
        let area = Rect::from_min_max(pos2(r.min.x + 24.0, r.min.y + 42.0), pos2(r.max.x - 24.0, r.max.y - 20.0));
        let (original, library) = (&self.pictures.original, &self.pictures.library);
        let scale = Place::fit(area, original.size().max(library.size())).scale;
        match view.mode {
            Mode::SideBySide => {
                let half = (area.width() - 24.0) / 2.0;
                let halves = [area.min.x, area.min.x + half + 24.0].map(|x| Rect::from_min_size(pos2(x, area.min.y), vec2(half, area.height())));
                for (shot, half, is_original, caption) in [(original, halves[0], true, tr("Original")), (library, halves[1], false, tr("Library"))] {
                    let place = Place { min: half.center() - shot.size() * scale / 2.0, scale };
                    painter.text(pos2(place.min.x, labels_y), Align2::LEFT_CENTER, caption, semibold(11.0), t.text_secondary);
                    pane(&painter, t, place, shot, None);
                    self.describe(ui, place, shot, is_original);
                }
            }
            Mode::Overlay => {
                let size = original.size().max(library.size());
                let place = Place::fit(area, size);
                let image = Rect::from_min_size(place.min, size * place.scale);
                let split_x = image.min.x + image.width() * view.split;
                pane(&painter, t, place, library, None);
                pane(&painter.with_clip_rect(Rect::from_min_max(r.min, pos2(split_x, r.max.y))), t, place, original, None);
                painter.text(pos2(image.min.x, labels_y), Align2::LEFT_CENTER, tr("Original"), semibold(11.0), t.text_secondary);
                painter.text(pos2(image.max.x, labels_y), Align2::RIGHT_CENTER, tr("Library"), semibold(11.0), t.text_secondary);
                painter.line_segment([pos2(split_x, image.min.y), pos2(split_x, image.max.y)], Stroke::new(2.0, ACCENT));
                painter.circle_filled(pos2(split_x, image.center().y), 6.0, ACCENT);
                self.describe(ui, place, original, true);
                self.describe(ui, place, library, false);
            }
            Mode::Difference => match self.difference.flatten() {
                Some(shot) => {
                    let place = Place::fit(area, shot.size());
                    let image = place.image(shot);
                    painter.text(
                        pos2(image.min.x, labels_y),
                        Align2::LEFT_CENTER,
                        tr("Differing pixels are shown in red"),
                        semibold(11.0),
                        t.text_secondary,
                    );
                    pane(&painter, t, place, shot, None);
                    a11y::image(
                        &ui.interact(image, ui.scope_id().with("difference"), Sense::hover()),
                        &trf("Difference of {variant}", &[("variant", &self.row.name)]),
                    );
                }
                None => message(ui, t, r, tr("The difference can't be drawn.")),
            },
        }
    }

    fn describe(&self, ui: &Ui, place: Place, shot: &Shot, original: bool) {
        let label = if original {
            trf("Original of {variant}", &[("variant", &self.row.name)])
        } else {
            trf("Library version of {variant}", &[("variant", &self.row.name)])
        };
        a11y::image(&ui.interact(place.image(shot), ui.scope_id().with(("picture", original)), Sense::hover()), &label);
    }
}
