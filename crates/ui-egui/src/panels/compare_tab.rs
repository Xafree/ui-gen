//! The Compare tab of the right panel: the imported component set selected, measured against the
//! original its import kept (`uigen.compare`, one variant per frame), one row per state. The
//! measure runs through `app.run`; everything else is read from the document.

use std::cell::OnceCell;
use std::sync::Arc;

use egui::{Color32, CornerRadius, Galley, Painter, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_engine::capture::JobState;
use figcraft_engine::{reference_of, uigen_compare};
use serde_json::{Value, json};

use crate::FigApp;
use crate::a11y;
use crate::compare_updates::{self, Updates};
use crate::compare_view::View;
use crate::dev_compare::{Place, Shot, pane};
use crate::i18n::{tr, trf};
use crate::theme::{Tokens, regular, semibold};
use crate::widgets::{self, PAD};

const COMPARE: &str = "uigen.compare";
const SOURCE_KEY: &str = "uigen.source";
const GAP: f32 = 8.0;
const MAX_IMAGE_H: f32 = 110.0;
const MAX_SCALE: f32 = 2.0;
const TOP: f32 = 10.0;
const LINE_GAP: f32 = 6.0;

pub fn available(app: &FigApp) -> bool {
    app.session.in_lib_project() && !cfg!(target_arch = "wasm32")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Source {
    pub package: String,
    pub version: String,
    pub component: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Ok,
    Ko,
    MissingReference,
    NotComparable,
    SizeMismatch,
}

impl Status {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "ok" => Self::Ok,
            "ko" => Self::Ko,
            "missingReference" => Self::MissingReference,
            "notComparable" => Self::NotComparable,
            "sizeMismatch" => Self::SizeMismatch,
            _ => return None,
        })
    }

    fn has_pictures(self) -> bool {
        matches!(self, Self::Ok | Self::Ko | Self::SizeMismatch | Self::NotComparable)
    }
}

pub(crate) struct Shapes {
    pub percent: f64,
    pub ok: bool,
}

pub(crate) struct TextVerdict {
    pub ok: bool,
    pub reason: String,
}

pub(crate) struct Pictures {
    pub original: Shot,
    pub library: Shot,
}

pub(crate) struct Row {
    pub variant: NodeId,
    pub name: String,
    pub state: Option<String>,
    pub axes: String,
    pub status: Status,
    pub reason: Option<String>,
    pub original: (u32, u32),
    pub library: (u32, u32),
    pub shapes: Option<Shapes>,
    pub text: Option<TextVerdict>,
    pub pictures: Option<Pictures>,
    pub difference: OnceCell<Option<Shot>>,
}

impl Row {
    pub(crate) fn title(&self) -> &str {
        self.state.as_deref().unwrap_or(&self.name)
    }

    fn too_large(&self) -> bool {
        self.reason.as_deref().is_some_and(|reason| reason.contains("too large"))
    }

    fn lacks_original(&self) -> bool {
        self.status == Status::MissingReference && !self.too_large()
    }
}

enum Phase {
    Empty,
    NotImported,
    Measuring(Vec<(NodeId, String)>),
    Failed(String),
    Done,
}

#[derive(Clone, PartialEq, Eq)]
struct Key {
    uid: u64,
    revision: u64,
    page: NodeId,
    set: Option<NodeId>,
    dark: bool,
    finished_job: u64,
}

pub(crate) struct Run {
    key: Key,
    phase: Phase,
    pub name: String,
    pub source: Option<Source>,
    pub rows: Vec<Row>,
}

impl Run {
    fn start(doc: &Document, key: Key) -> Self {
        let mut run = Self { phase: Phase::Empty, name: String::new(), source: None, rows: Vec::new(), key };
        let Some(set) = run.key.set else { return run };
        run.name = doc.node(set).map(|node| node.name.clone()).unwrap_or_default();
        run.source = source_of(doc, set);
        run.phase = if run.source.is_some() { Phase::Measuring(variants_of(doc, set)) } else { Phase::NotImported };
        run
    }

    pub(crate) fn set(&self) -> Option<NodeId> {
        self.key.set
    }

    fn knows(&self, variant: NodeId) -> bool {
        match &self.phase {
            Phase::Measuring(queue) => queue.iter().any(|(id, _)| *id == variant),
            _ => self.rows.iter().any(|row| row.variant == variant),
        }
    }
}

#[derive(Default)]
pub struct CompareTab {
    dark: bool,
    run: Option<Run>,
    pub(crate) detail: Option<NodeId>,
    pub(crate) view: View,
    updates: Updates,
}

impl CompareTab {
    pub(crate) fn run(&self) -> Option<&Run> {
        self.run.as_ref()
    }

    pub(crate) fn dark(&self) -> bool {
        self.dark
    }
}

enum Act {
    Theme(bool),
    Recapture,
    Open(NodeId),
}

fn source_of(doc: &Document, set: NodeId) -> Option<Source> {
    let value: Value = serde_json::from_str(doc.node(set)?.plugin_data.get(SOURCE_KEY)?).ok()?;
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    Some(Source { package: text("package")?, version: text("version")?, component: text("component")? })
}

fn variants_of(doc: &Document, set: NodeId) -> Vec<(NodeId, String)> {
    let Some(node) = doc.node(set) else { return Vec::new() };
    node.children
        .iter()
        .filter_map(|child| {
            doc.node(*child).filter(|variant| matches!(variant.kind, NodeKind::Component { .. })).map(|variant| (*child, variant.name.clone()))
        })
        .collect()
}

fn set_of(doc: &Document, selection: &[NodeId]) -> Option<NodeId> {
    let mut at = selection.first().copied();
    while let Some(id) = at {
        let node = doc.node(id)?;
        if matches!(node.kind, NodeKind::ComponentSet { .. }) {
            return Some(id);
        }
        at = node.parent;
    }
    None
}

fn latest_finished_job(app: &FigApp) -> u64 {
    app.session
        .capture_jobs()
        .iter()
        .filter(|job| matches!(job.state, JobState::Done | JobState::Failed(_)))
        .map(|job| job.job)
        .max()
        .unwrap_or_default()
}

pub fn show(app: &mut FigApp, ui: &mut Ui) {
    let mut tab = std::mem::take(&mut app.ui.compare);
    follow(app, &mut tab);
    paint(app, ui, &mut tab);
    advance(app, ui.ctx(), &mut tab);
    app.ui.compare = tab;
}

fn follow(app: &FigApp, tab: &mut CompareTab) {
    let finished_job = latest_finished_job(app);
    let Ok(ds) = app.session.ds() else {
        tab.run = None;
        return;
    };
    let set = set_of(&ds.doc, &ds.selection);
    let key = Key { uid: ds.uid, revision: ds.revision, page: ds.page, set, dark: tab.dark, finished_job };
    if tab.run.as_ref().is_none_or(|run| run.key != key) {
        let run = Run::start(&ds.doc, key);
        tab.detail = tab.detail.filter(|variant| run.knows(*variant));
        tab.run = Some(run);
    }
}

fn advance(app: &mut FigApp, ctx: &egui::Context, tab: &mut CompareTab) {
    let dark = tab.dark;
    let Some(run) = tab.run.as_mut() else { return };
    let Phase::Measuring(queue) = &run.phase else { return };
    let total = queue.len();
    let (Some((variant, name)), Some(set)) = (queue.get(run.rows.len()).cloned(), run.set()) else {
        run.phase = Phase::Done;
        return;
    };
    let params = json!({"set": set.0, "theme": if dark { "dark" } else { "light" }, "variants": [name]});
    let outcome = match app.run(COMPARE, params) {
        Some(result) => measured(app, ctx, &result, variant, &name, dark).ok_or_else(String::new),
        None => Err(app.ui.last_error.clone().unwrap_or_default()),
    };
    match outcome {
        Ok(row) => run.rows.push(row),
        Err(cause) => run.phase = Phase::Failed(cause),
    }
    if matches!(run.phase, Phase::Measuring(_)) && run.rows.len() == total {
        run.phase = Phase::Done;
    }
    ctx.request_repaint();
}

fn measured(app: &FigApp, ctx: &egui::Context, result: &Value, variant: NodeId, name: &str, dark: bool) -> Option<Row> {
    let entry = result.get("rows")?.as_array()?.first()?;
    let mut row = row_of(entry, variant, name)?;
    if row.status.has_pictures() {
        row.pictures = pictures(app, ctx, variant, dark);
    }
    Some(row)
}

fn row_of(entry: &Value, variant: NodeId, name: &str) -> Option<Row> {
    let status = Status::parse(entry.get("status")?.as_str()?)?;
    let text_of = |key: &str| entry.get(key).and_then(Value::as_str).map(str::to_owned);
    let size = |key: &str| {
        let side = |axis: &str| {
            entry.get(key).and_then(|size| size.get(axis)).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).unwrap_or_default()
        };
        (side("width"), side("height"))
    };
    let object = |key: &str| entry.get(key).filter(|value| value.is_object());
    let shapes = object("shapes").map(|s| Shapes {
        percent: s.get("percent").and_then(Value::as_f64).unwrap_or_default(),
        ok: s.get("verdict").and_then(Value::as_str) == Some("ok"),
    });
    let text = object("text").map(|x| TextVerdict {
        ok: x.get("verdict").and_then(Value::as_str) == Some("ok"),
        reason: x.get("reason").and_then(Value::as_str).unwrap_or_default().to_owned(),
    });
    let axes: Vec<String> = [text_of("style"), text_of("size")].into_iter().flatten().collect();
    Some(Row {
        variant,
        name: name.to_owned(),
        state: text_of("state"),
        axes: axes.join(" · "),
        status,
        reason: text_of("reason"),
        original: size("original"),
        library: size("library"),
        shapes,
        text,
        pictures: None,
        difference: OnceCell::new(),
    })
}

pub(crate) fn original_raster(app: &FigApp, variant: NodeId, dark: bool) -> Option<uigen_compare::Raster> {
    let ds = app.session.ds().ok()?;
    let root = &app.session.project()?.root;
    let theme = if dark { "dark" } else { "light" };
    let png = reference_of(&ds.doc, variant)?.themes.get(theme)?.png.clone()?;
    uigen_compare::original_raster(root, &png).ok()
}

fn pictures(app: &FigApp, ctx: &egui::Context, variant: NodeId, dark: bool) -> Option<Pictures> {
    let ds = app.session.ds().ok()?;
    let original = Shot::from_raster(ctx, &original_raster(app, variant, dark)?)?;
    let themed = uigen_compare::themed(&ds.doc, variant, dark);
    let library = Shot::new(ctx, &themed, variant, std::iter::empty())?;
    Some(Pictures { original, library })
}

fn paint(app: &mut FigApp, ui: &mut Ui, tab: &mut CompareTab) {
    let t = Tokens::get(ui.ctx());
    let mut acts = Vec::new();
    if let Some(run) = tab.run() {
        body(ui, &t, run, tab.dark(), &mut acts);
    }
    let target = tab.run.as_ref().and_then(|run| Some((run.set()?, run.source.as_ref()?.package.clone())));
    if let Some((set, package)) = target {
        compare_updates::section(app, ui, &mut tab.updates, set, &package);
    }
    ui.add_space(24.0);
    for act in acts {
        match act {
            Act::Theme(dark) => tab.dark = dark,
            Act::Open(variant) => tab.detail = Some(variant),
            Act::Recapture => {
                if let Some(source) = tab.run.as_ref().and_then(|run| run.source.as_ref()) {
                    compare_updates::recapture_source(app, &mut tab.updates, &source.package);
                }
            }
        }
    }
}

fn body(ui: &mut Ui, t: &Tokens, run: &Run, dark: bool, acts: &mut Vec<Act>) {
    match &run.phase {
        Phase::Empty => widgets::note(ui, tr("Select an imported component to compare it with its original."), t.text_secondary),
        Phase::NotImported => widgets::note(ui, tr("This component was not imported: there is no original to compare."), t.text_secondary),
        Phase::Failed(cause) => {
            theme_selector(ui, dark, acts);
            widgets::note(ui, &trf("Couldn't compare {name}: {cause}", &[("name", &run.name), ("cause", cause)]), t.danger);
        }
        Phase::Measuring(queue) => {
            theme_selector(ui, dark, acts);
            widgets::note(ui, &trf("Measuring {done} of {total}…", &[("done", &run.rows.len()), ("total", &queue.len())]), t.text_secondary);
            rows(ui, t, run, acts);
        }
        Phase::Done => {
            theme_selector(ui, dark, acts);
            if dark && !run.rows.is_empty() && run.rows.iter().all(|row| row.status == Status::NotComparable) {
                widgets::note(ui, tr("Dark mode can't be compared: the library does not follow the system colour scheme."), t.text_secondary);
                return;
            }
            if run.rows.iter().any(Row::lacks_original) {
                widgets::note(ui, tr("The original capture is missing. Recapture the source to compare."), t.text_secondary);
                crate::right::row(ui, 32.0, |ui| {
                    if widgets::button(ui, tr("Recapture"), true, true).clicked() {
                        acts.push(Act::Recapture);
                    }
                });
            }
            rows(ui, t, run, acts);
        }
    }
}

fn theme_selector(ui: &mut Ui, dark: bool, acts: &mut Vec<Act>) {
    crate::right::block(ui, 44.0, 10.0, |ui| {
        if let Some(picked) = widgets::segmented_text(ui, &[tr("Light"), tr("Dark")], usize::from(dark)) {
            acts.push(Act::Theme(picked == 1));
        }
    });
}

fn rows(ui: &mut Ui, t: &Tokens, run: &Run, acts: &mut Vec<Act>) {
    for row in &run.rows {
        row_card(ui, t, row, acts);
    }
}

struct Pill {
    label: Arc<Galley>,
    badge: Arc<Galley>,
    color: Color32,
    description: String,
}

struct Plan {
    title: Arc<Galley>,
    axes: Option<Arc<Galley>>,
    shapes: Option<Pill>,
    text: Option<Pill>,
    notice: Option<Arc<Galley>>,
    size_note: Option<Arc<Galley>>,
    scale: f32,
    height: f32,
}

fn pill(painter: &Painter, t: &Tokens, label: &str, ok: bool, description: &str) -> Pill {
    let color = if ok { t.success } else { t.danger };
    let badge = painter.layout_no_wrap((if ok { tr("OK") } else { tr("Differs") }).to_owned(), semibold(10.0), color);
    Pill { label: painter.layout_no_wrap(label.to_owned(), regular(10.0), t.text_secondary), badge, color, description: description.to_owned() }
}

fn notice_of(row: &Row) -> Option<String> {
    match row.status {
        Status::NotComparable => row.reason.clone(),
        Status::MissingReference if row.too_large() => Some(tr("The original capture is too large to compare.").to_owned()),
        _ => None,
    }
}

pub(crate) fn dimensions(size: (u32, u32)) -> String {
    format!("{} × {}", size.0, size.1)
}

fn plan(ui: &Ui, t: &Tokens, row: &Row, width: f32) -> Plan {
    let painter = ui.painter();
    let wrapped = |text: String| painter.layout(text, regular(10.0), t.text_secondary, width.max(40.0));
    let title = painter.layout_no_wrap(row.title().to_owned(), semibold(12.0), t.text);
    let axes = (row.state.is_some() && !row.axes.is_empty()).then(|| painter.layout_no_wrap(row.axes.clone(), regular(10.0), t.text_secondary));
    let shapes = row.shapes.as_ref().map(|s| {
        let figure = trf("Shapes {percent}", &[("percent", &format!("{:.1} %", s.percent))]);
        pill(painter, t, &figure, s.ok, &figure)
    });
    let text = row.text.as_ref().map(|x| pill(painter, t, tr("Text"), x.ok, &x.reason));
    let notice = notice_of(row).map(wrapped);
    let size_note = row.pictures.as_ref().filter(|_| row.original != row.library).map(|_| {
        wrapped(trf(
            "Size differs: original {original}, library {library}",
            &[("original", &dimensions(row.original)), ("library", &dimensions(row.library))],
        ))
    });
    let scale = row.pictures.as_ref().map_or(1.0, |pictures| {
        let (original, library) = (pictures.original.size(), pictures.library.size());
        let across = (width - GAP) / (original.x + library.x).max(1.0);
        let down = MAX_IMAGE_H / original.y.max(library.y).max(1.0);
        across.min(down).clamp(0.05, MAX_SCALE)
    });
    let mut height = TOP + title.size().y + LINE_GAP;
    height += axes.as_ref().map_or(0.0, |g| g.size().y + LINE_GAP);
    if let Some(pictures) = &row.pictures {
        let tallest = pictures.original.size().y.max(pictures.library.size().y) * scale;
        height += 14.0 + tallest + LINE_GAP;
        height += text.as_ref().map_or(0.0, |_| 22.0 + LINE_GAP);
    }
    height += size_note.as_ref().map_or(0.0, |g| g.size().y + LINE_GAP);
    height += notice.as_ref().map_or(0.0, |g| g.size().y + LINE_GAP);
    Plan { title, axes, shapes, text, notice, size_note, scale, height: height + TOP }
}

fn row_card(ui: &mut Ui, t: &Tokens, row: &Row, acts: &mut Vec<Act>) {
    let width = ui.available_width();
    let plan = plan(ui, t, row, width - 2.0 * PAD);
    let (rect, _) = ui.allocate_exact_size(vec2(width, plan.height), Sense::hover());
    let id = ui.scope_id().with(("compare_row", row.variant));
    let openable = row.pictures.is_some();
    let resp = ui.interact(rect, id, if openable { Sense::click() } else { Sense::hover() });
    if openable {
        a11y::named_button(&resp, &row.name);
        if resp.clicked() {
            acts.push(Act::Open(row.variant));
        }
    }
    let painter = ui.painter().clone();
    if resp.hovered() && openable {
        painter.rect_filled(rect.shrink2(vec2(8.0, 0.0)), CornerRadius::same(5), t.hover);
    }
    let x = rect.min.x + PAD;
    let mut y = rect.min.y + TOP;
    painter.galley(pos2(x, y), plan.title.clone(), t.text);
    if let Some(shapes) = &plan.shapes {
        let size = badge_size(&shapes.badge);
        let badge = Rect::from_min_size(pos2(rect.max.x - PAD - size.x, y + (plan.title.size().y - size.y) / 2.0), size);
        paint_pill(ui, &painter, id.with("shapes"), shapes, badge, tr("Shapes"));
        let label_at = pos2(badge.min.x - GAP - shapes.label.size().x, y + (plan.title.size().y - shapes.label.size().y) / 2.0);
        painter.galley(label_at, shapes.label.clone(), t.text_secondary);
    }
    y += plan.title.size().y + LINE_GAP;
    if let Some(axes) = &plan.axes {
        painter.galley(pos2(x, y), axes.clone(), t.text_secondary);
        y += axes.size().y + LINE_GAP;
    }
    if row.pictures.is_some() {
        y = paint_pictures(ui, &painter, t, id, row, plan.scale, pos2(x, y));
        if let Some(text) = &plan.text {
            let size = badge_size(&text.badge);
            painter.galley(pos2(x, y + (size.y - text.label.size().y) / 2.0), text.label.clone(), t.text_secondary);
            let badge = Rect::from_min_size(pos2(x + text.label.size().x + GAP, y), size);
            paint_pill(ui, &painter, id.with("text"), text, badge, tr("Text"));
            y += 22.0 + LINE_GAP;
        }
    }
    for note in [&plan.size_note, &plan.notice].into_iter().flatten() {
        painter.galley(pos2(x, y), note.clone(), t.text_secondary);
        y += note.size().y + LINE_GAP;
    }
    painter.line_segment([pos2(rect.min.x, rect.max.y - 0.5), pos2(rect.max.x, rect.max.y - 0.5)], Stroke::new(1.0, t.border));
}

fn badge_size(badge: &Galley) -> egui::Vec2 {
    badge.size() + vec2(10.0, 4.0)
}

fn paint_pill(ui: &Ui, painter: &Painter, id: egui::Id, pill: &Pill, rect: Rect, name: &str) {
    painter.rect_filled(rect, CornerRadius::same(4), pill.color.gamma_multiply(0.15));
    painter.galley(rect.center() - pill.badge.size() / 2.0, pill.badge.clone(), pill.color);
    let resp = ui.interact(rect, id, Sense::hover());
    a11y::status(&resp, &format!("{name} {}", pill.badge.text()), &pill.description);
    if !pill.description.is_empty() {
        resp.on_hover_text(&pill.description);
    }
}

fn paint_pictures(ui: &Ui, painter: &Painter, t: &Tokens, id: egui::Id, row: &Row, scale: f32, at: Pos2) -> f32 {
    let Some(pictures) = &row.pictures else { return at.y };
    let captions_y = at.y;
    let top = at.y + 14.0;
    let original = Place { min: pos2(at.x, top), scale };
    let original_rect = original.image(&pictures.original);
    let library = Place { min: pos2(original_rect.max.x + GAP, top), scale };
    let library_rect = library.image(&pictures.library);
    for (caption, rect) in [(tr("Original"), original_rect), (tr("Library"), library_rect)] {
        painter.text(pos2(rect.min.x, captions_y), egui::Align2::LEFT_TOP, caption, regular(10.0), t.text_secondary);
    }
    pane(painter, t, original, &pictures.original, None);
    pane(painter, t, library, &pictures.library, None);
    let described = [
        (id.with("original"), original_rect, trf("Original of {variant}", &[("variant", &row.name)])),
        (id.with("library"), library_rect, trf("Library version of {variant}", &[("variant", &row.name)])),
    ];
    for (picture_id, rect, label) in described {
        a11y::image(&ui.interact(rect, picture_id, Sense::hover()), &label);
    }
    original_rect.max.y.max(library_rect.max.y) + LINE_GAP
}
