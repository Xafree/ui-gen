use std::path::Path;

use figcraft_doc::{CollectionId, Document, ModeId, NodeId, NodeKind, TextAutoResize, TextStyle};
use figcraft_geom::Rect;
use figcraft_render::{images, render_node, to_rgba8};
use serde::Serialize;
use serde_json::Value;

use super::{
    LibText, PixelReport, Raster, RefText, SHAPES_MAX_PERCENT, TextBox, TextReport, Verdict, pixel_diff, text_verdict, weight_of_font_style,
};
use crate::file_access;
use crate::uigen_import::{SOURCE_KEY, Theme, reference_of};

pub(crate) const MAX_VARIANTS: usize = 128;
const MAX_PNG_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SIDE: u32 = 4096;
const MAX_LIBRARY_TEXTS: usize = 1000;

const TOO_LARGE: &str = "original too large to compare (more than 8 MiB or 4096 × 4096 pixels)";
const UNREADABLE: &str = "the original image is missing or can't be read";
const NO_REFERENCE: &str = "no original was kept for this variant";
const NO_IMAGE: &str = "the path of the original image was not kept";
const NOT_DISTINCT: &str = "the library does not follow the system colour scheme: its dark render is its light render";
const LIBRARY_TOO_LARGE: &str = "the variant is larger than 4096 × 4096 pixels";
const NOT_RENDERED: &str = "the variant can't be rendered";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
    pub package: String,
    pub version: String,
    pub component: String,
}

pub(crate) struct Subject {
    pub set: NodeId,
    pub name: String,
    pub source: Source,
    pub variants: Vec<(NodeId, String)>,
}

fn source_of(doc: &Document, set: NodeId) -> Option<Source> {
    let value: Value = serde_json::from_str(doc.node(set)?.plugin_data.get(SOURCE_KEY)?).ok()?;
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    Some(Source { package: text("package")?, version: text("version")?, component: text("component")? })
}

fn imported_set_of_page(doc: &Document, page: NodeId) -> Option<NodeId> {
    let children = &doc.node(page)?.children;
    children
        .iter()
        .copied()
        .find(|child| doc.node(*child).is_some_and(|set| matches!(set.kind, NodeKind::ComponentSet { .. })) && source_of(doc, *child).is_some())
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

pub(crate) fn subject(doc: &Document, set: Option<NodeId>, page: NodeId, wanted: Option<&[String]>) -> Result<Subject, String> {
    let set = match set {
        Some(set) => set,
        None => imported_set_of_page(doc, page).ok_or("the current page holds no imported component set: give `set`")?,
    };
    let node = doc.node(set).ok_or_else(|| format!("`set` {set} does not exist"))?;
    if !matches!(node.kind, NodeKind::ComponentSet { .. }) {
        return Err(format!("`set` {set} is a {}: only a component set can be compared", node.kind.type_name()));
    }
    let source = source_of(doc, set).ok_or_else(|| format!("`set` {set} has no uigen.source: it was not imported from a library"))?;
    let mut variants = variants_of(doc, set);
    if variants.len() > MAX_VARIANTS {
        return Err(format!("`set` {set} has {} variants: at most {MAX_VARIANTS} can be compared", variants.len()));
    }
    if let Some(wanted) = wanted {
        if let Some(unknown) = wanted.iter().find(|name| !variants.iter().any(|(_, known)| known == *name)) {
            return Err(format!("`variants` names \"{unknown}\", which is not a variant of `set` {set}"));
        }
        variants.retain(|(_, name)| wanted.contains(name));
    }
    Ok(Subject { set, name: node.name.clone(), source, variants })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Status {
    Ok,
    Ko,
    MissingReference,
    NotComparable,
    SizeMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Judgement {
    Ok,
    Ko,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Shapes {
    pub differing: u64,
    pub total: u64,
    pub percent: f64,
    pub verdict: Judgement,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Text {
    pub verdict: Judgement,
    pub reason: String,
    pub max_dx: f64,
    pub max_dy: f64,
    pub max_dwidth: f64,
    pub pixel_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Row {
    pub variant: String,
    pub style: Option<String>,
    pub size: Option<String>,
    pub state: Option<String>,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub original: Size,
    pub library: Size,
    pub shapes: Option<Shapes>,
    pub text: Option<Text>,
}

fn axis(name: &str, key: &str) -> Option<String> {
    name.split(", ").find_map(|part| part.strip_prefix(key)?.strip_prefix('=')).map(str::to_owned)
}

fn finite(number: f64) -> f64 {
    if number.is_finite() { number } else { 0.0 }
}

fn whole(length: f64) -> u32 {
    if length.is_finite() && length > 0.0 { length.round().min(f64::from(u32::MAX)) as u32 } else { 0 }
}

fn size_of(raster: &Raster) -> Size {
    Size { width: raster.width, height: raster.height }
}

fn judgement(verdict: Verdict) -> Judgement {
    match verdict {
        Verdict::Ok => Judgement::Ok,
        Verdict::Ko => Judgement::Ko,
    }
}

impl Row {
    fn new(name: &str, library: Size) -> Row {
        Row {
            variant: name.to_owned(),
            style: axis(name, "Style"),
            size: axis(name, "Size"),
            state: axis(name, "State"),
            status: Status::NotComparable,
            reason: None,
            original: Size::default(),
            library,
            shapes: None,
            text: None,
        }
    }

    fn without(mut self, status: Status, reason: &str) -> Row {
        self.status = status;
        self.reason = Some(reason.to_owned());
        self
    }

    fn measured(mut self, pixels: &PixelReport, text: &TextReport) -> Row {
        let percent = finite(pixels.shapes_percent);
        let shapes_ok = percent < SHAPES_MAX_PERCENT;
        self.status = match (pixels.size_mismatch, shapes_ok && text.verdict == Verdict::Ok) {
            (true, _) => Status::SizeMismatch,
            (false, true) => Status::Ok,
            (false, false) => Status::Ko,
        };
        self.shapes = Some(Shapes {
            differing: pixels.differing_shapes,
            total: pixels.shapes_total,
            percent,
            verdict: if shapes_ok { Judgement::Ok } else { Judgement::Ko },
        });
        self.text = Some(Text {
            verdict: judgement(text.verdict),
            reason: text.reason.clone(),
            max_dx: finite(text.max_dx),
            max_dy: finite(text.max_dy),
            max_dwidth: finite(text.max_dwidth),
            pixel_percent: finite(pixels.text_percent),
        });
        self
    }
}

fn read_original(root: &str, png: &str) -> Result<Raster, &'static str> {
    let path = Path::new(root).join(png);
    let found = file_access::metadata(&path).map_err(|error| {
        log::debug!("uigen.compare: {png}: {error}");
        UNREADABLE
    })?;
    if !found.is_file() {
        return Err(UNREADABLE);
    }
    if found.len() > MAX_PNG_BYTES {
        return Err(TOO_LARGE);
    }
    let bytes = file_access::read_capped(&path, MAX_PNG_BYTES).map_err(|error| {
        log::debug!("uigen.compare: {png}: {error}");
        UNREADABLE
    })?;
    let (width, height) = images::dimensions(&bytes).ok_or(UNREADABLE)?;
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(TOO_LARGE);
    }
    let pixels = images::decode(&bytes).ok_or(UNREADABLE)?;
    Raster::new(pixels.width(), pixels.height(), to_rgba8(&pixels)).map_err(|_| UNREADABLE)
}

fn follow(scratch: &mut Document, variant: NodeId, theme: Theme) {
    let wanted = theme.mode();
    let modes: Vec<(CollectionId, ModeId)> = scratch
        .variables
        .collections
        .iter()
        .filter_map(|collection| {
            let mode = collection.modes.iter().find(|mode| mode.name.trim().eq_ignore_ascii_case(wanted))?;
            Some((collection.id.clone(), mode.id.clone()))
        })
        .collect();
    let before = scratch.clone();
    if let Some(node) = scratch.node_mut(variant) {
        node.var_modes.extend(modes);
    }
    crate::normalize(scratch, &before);
}

fn render_library(scratch: &Document, variant: NodeId) -> Result<Raster, &'static str> {
    let bounds = figcraft_render::export_bounds(scratch, variant).ok_or(NOT_RENDERED)?;
    if !(bounds.width() <= f64::from(MAX_SIDE) && bounds.height() <= f64::from(MAX_SIDE)) {
        return Err(LIBRARY_TOO_LARGE);
    }
    let pixels = render_node(scratch, variant, 1.0).ok_or(NOT_RENDERED)?;
    Raster::new(pixels.width(), pixels.height(), to_rgba8(&pixels)).map_err(|_| NOT_RENDERED)
}

fn weight_of(style: &TextStyle) -> u16 {
    match style.variations.get("wght") {
        Some(weight) if weight.is_finite() && (1.0..=1000.0).contains(weight) => weight.round() as u16,
        _ => weight_of_font_style(&style.font_style),
    }
}

fn library_text(doc: &Document, id: NodeId, origin: &Rect) -> Option<LibText> {
    let node = doc.node(id).filter(|node| node.visible)?;
    let NodeKind::Text(data) = &node.kind else { return None };
    let at = doc.world_bounds(id)?;
    let measured = data.auto_resize == TextAutoResize::WidthAndHeight;
    let width = if measured { figcraft_text::auto_size(node).map_or(node.width, |(width, _)| width) } else { node.width };
    let family = data.style.font_family.clone();
    Some(LibText {
        installed: figcraft_text::has_family(&family),
        family,
        size: data.style.font_size,
        weight: weight_of(&data.style),
        x: at.x0 - origin.x0,
        y: at.y0 - origin.y0,
        width,
        width_measured: measured,
    })
}

fn library_texts(doc: &Document, variant: NodeId) -> Vec<LibText> {
    let Some(origin) = doc.world_bounds(variant) else { return Vec::new() };
    doc.subtree(variant).into_iter().filter_map(|id| library_text(doc, id, &origin)).take(MAX_LIBRARY_TEXTS).collect()
}

fn boxes_of(texts: &[RefText]) -> Vec<TextBox> {
    texts.iter().map(|text| TextBox { box_: text.box_, line: text.line }).collect()
}

struct Context<'a> {
    doc: &'a Document,
    scratch: Document,
    root: &'a str,
    theme: Theme,
}

impl Context<'_> {
    fn row(&mut self, id: NodeId, name: &str) -> Row {
        let size = self.doc.node(id).map_or_else(Size::default, |node| Size { width: whole(node.width), height: whole(node.height) });
        let row = Row::new(name, size);
        let Some(reference) = reference_of(self.doc, id) else { return row.without(Status::MissingReference, NO_REFERENCE) };
        let Some(kept) = reference.themes.get(self.theme.render()) else {
            return row.without(Status::MissingReference, &format!("no {} render of the original was kept", self.theme.render()));
        };
        let mut row = row;
        row.original = Size { width: kept.width, height: kept.height };
        if self.theme == Theme::Dark && !kept.distinct {
            return row.without(Status::NotComparable, NOT_DISTINCT);
        }
        let Some(png) = kept.png.as_deref() else { return row.without(Status::MissingReference, NO_IMAGE) };
        let original = match read_original(self.root, png) {
            Ok(original) => original,
            Err(reason) => return row.without(Status::MissingReference, reason),
        };
        row.original = size_of(&original);
        follow(&mut self.scratch, id, self.theme);
        let library = match render_library(&self.scratch, id) {
            Ok(library) => library,
            Err(reason) => return row.without(Status::NotComparable, reason),
        };
        row.library = size_of(&library);
        let pixels = pixel_diff(&original, &library, &boxes_of(&kept.texts));
        let text = text_verdict(&kept.texts, &library_texts(&self.scratch, id));
        row.measured(&pixels, &text)
    }
}

pub(crate) fn measure(doc: &Document, root: &str, theme: Theme, variants: &[(NodeId, String)]) -> Vec<Row> {
    figcraft_text::scan_system_fonts();
    let mut context = Context { doc, scratch: doc.clone(), root, theme };
    variants.iter().map(|(id, name)| context.row(*id, name)).collect()
}
