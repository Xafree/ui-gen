//! The original of an imported variant: what the capture service rendered, kept as a light reference
//! on the variant (`uigen.reference`: the image path, its size and the text boxes of each theme) and
//! the options of the import on the set (`uigen.importOptions`). The image itself stays on disk.

use std::collections::BTreeMap;

use figcraft_doc::{Document, NodeId};
use figcraft_dom::{DomBox, DomNode, Snapshot, TextFace, text_faces};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::assemble::Variant;
use super::pending::Request;
use crate::uigen_compare::RefText;

pub const REFERENCE_KEY: &str = "uigen.reference";
pub const OPTIONS_KEY: &str = "uigen.importOptions";

const MAX_TEXTS: usize = 64;
const MAX_BYTES: usize = 16 * 1024;
const MAX_PNG_CHARS: usize = 512;
const CAPTURES_DIR: &str = ".uigen/captures/";
const SHORT_FAMILY_CHARS: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeReference {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png: Option<String>,
    pub width: u32,
    pub height: u32,
    pub distinct: bool,
    pub texts: Vec<RefText>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub package: String,
    pub version: String,
    pub component: String,
    pub state: String,
    pub themes: BTreeMap<String, ThemeReference>,
}

impl RefText {
    fn is_finite(&self) -> bool {
        self.size.is_finite() && self.box_.iter().chain(self.line.iter().flatten()).all(|number| number.is_finite())
    }
}

impl ThemeReference {
    fn is_sound(&self) -> bool {
        self.png.as_deref().is_none_or(is_capture_path) && self.texts.len() <= MAX_TEXTS && self.texts.iter().all(RefText::is_finite)
    }
}

/// Whether `path` is an image the capture service wrote: relative to the project, under the captures
/// folder, nothing that leaves it.
fn is_capture_path(path: &str) -> bool {
    let Some(name) = path.strip_prefix(CAPTURES_DIR) else { return false };
    !name.is_empty()
        && path.chars().count() <= MAX_PNG_CHARS
        && !path.chars().any(|c| c.is_control() || matches!(c, '\\' | ':'))
        && !path.split('/').any(|segment| segment.is_empty() || segment == "." || segment == "..")
}

/// The original kept on `variant`; unreadable, oversized or unsound JSON reads as no reference.
pub fn reference_of(doc: &Document, variant: NodeId) -> Option<Reference> {
    let raw = doc.node(variant)?.plugin_data.get(REFERENCE_KEY)?;
    if raw.len() > MAX_BYTES {
        return None;
    }
    let reference: Reference = serde_json::from_str(raw).ok()?;
    reference.themes.values().all(ThemeReference::is_sound).then_some(reference)
}

pub(super) struct Kept {
    pub raw: Option<String>,
    pub warnings: Vec<String>,
}

/// The reference of `variant`: for each theme of the request the image the service rendered, the size
/// of the root box and the text boxes with their fonts, in tree order.
pub(super) fn keep(request: &Request, variant: &Variant) -> Kept {
    let mut warnings = Vec::new();
    let mut note = |message: String| warnings.push(format!("{}: {message}", variant.label));
    let mut themes = BTreeMap::new();
    for (index, theme) in request.themes.iter().enumerate() {
        let snapshot = index.checked_sub(1).map_or(Some(&variant.snapshot), |other| variant.others.get(other));
        let Some(snapshot) = snapshot else { continue };
        let name = theme.render();
        let png = variant.pngs.get(index).and_then(Option::as_deref).filter(|path| is_capture_path(path));
        if png.is_none() {
            note(format!("the {name} render has no usable capture path: its original image is not kept"));
        }
        let (texts, left_out, unplaced) = texts_of(snapshot);
        if left_out {
            note(format!("more than {MAX_TEXTS} texts in the {name} render: the first {MAX_TEXTS} are kept in its original"));
        }
        if unplaced > 0 {
            note(format!("{unplaced} texts of the {name} render have a position that can't be kept: left out of its original"));
        }
        let root = root_box(&snapshot.root);
        let (width, height) = (pixels(root.width), pixels(root.height));
        let distinct = index == 0 || *snapshot != variant.snapshot;
        themes.insert(name.to_string(), ThemeReference { png: png.map(str::to_string), width, height, distinct, texts });
    }
    let mut reference = Reference {
        package: request.package.clone(),
        version: request.version.clone(),
        component: request.component.clone(),
        state: variant.state.label().to_string(),
        themes,
    };
    let raw = fit(&mut reference);
    match &raw {
        Some((_, true)) => {
            note(format!("its original is larger than {} KiB: font names are shortened and the last texts are left out", MAX_BYTES / 1024))
        }
        None => note("its original could not be kept".to_string()),
        Some((_, false)) => {}
    }
    Kept { raw: raw.map(|(raw, _)| raw), warnings }
}

/// The texts of one render (at most `MAX_TEXTS`), whether more were left out, and how many could not
/// be placed relative to the root box.
fn texts_of(snapshot: &Snapshot) -> (Vec<RefText>, bool, usize) {
    let origin = root_box(&snapshot.root);
    let faces = text_faces(snapshot);
    let mut texts: Vec<RefText> = faces.iter().filter_map(|face| ref_text(face, &origin)).collect();
    let unplaced = faces.len() - texts.len();
    let left_out = texts.len() > MAX_TEXTS;
    texts.truncate(MAX_TEXTS);
    (texts, left_out, unplaced)
}

fn ref_text(face: &TextFace, origin: &DomBox) -> Option<RefText> {
    let relative = |dom_box: &DomBox| [dom_box.x - origin.x, dom_box.y - origin.y, dom_box.width, dom_box.height];
    let text = RefText {
        box_: relative(&face.text_box),
        line: face.line_box.as_ref().map(relative),
        family: face.family.clone(),
        size: face.size,
        weight: face.weight,
    };
    text.is_finite().then_some(text)
}

fn root_box(node: &DomNode) -> DomBox {
    match node {
        DomNode::Element(element) => element.r#box,
        DomNode::Text(run) => run.r#box,
        DomNode::Svg(svg) => svg.r#box,
    }
}

fn pixels(length: f64) -> u32 {
    if length.is_finite() && length > 0.0 { length.round().min(f64::from(u32::MAX)) as u32 } else { 0 }
}

/// The reference as JSON within `MAX_BYTES`, and whether it had to be cut: font names shortened
/// first, then the last texts of the fullest theme one by one. `None` when nothing fits.
fn fit(reference: &mut Reference) -> Option<(String, bool)> {
    let raw = serde_json::to_string(reference).ok()?;
    if raw.len() <= MAX_BYTES {
        return Some((raw, false));
    }
    for text in reference.themes.values_mut().flat_map(|theme| theme.texts.iter_mut()) {
        text.family = text.family.chars().take(SHORT_FAMILY_CHARS).collect();
    }
    let mut raw = serde_json::to_string(reference).ok()?;
    while raw.len() > MAX_BYTES {
        reference.themes.values_mut().max_by_key(|theme| theme.texts.len())?.texts.pop()?;
        raw = serde_json::to_string(reference).ok()?;
    }
    Some((raw, true))
}

/// The options of the call, defaults included: the values each axis was imported with.
pub(super) fn options_of(request: &Request, variants: &[Variant]) -> String {
    let styles = unique(variants.iter().filter_map(|variant| variant.style.as_deref()));
    let sizes = unique(variants.iter().filter_map(|variant| variant.size.as_deref()));
    let states = unique(variants.iter().map(|variant| variant.state.label()));
    let themes: Vec<&str> = request.themes.iter().map(|theme| theme.render()).collect();
    json!({
        "styleProp": request.style_prop,
        "sizeProp": request.size_prop,
        "styles": styles,
        "sizes": sizes,
        "states": states,
        "themes": themes,
        "animations": request.animations,
    })
    .to_string()
}

fn unique<'a>(values: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut seen: Vec<&str> = Vec::new();
    for value in values {
        if !seen.contains(&value) {
            seen.push(value);
        }
    }
    seen
}
