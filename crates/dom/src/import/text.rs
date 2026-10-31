//! A text run becomes a text layer at its box, in the font its parent element computed.

use figcraft_color::{Color, Paint};
use figcraft_doc::geom::Affine;
use figcraft_doc::{LetterSpacing, LineHeight, Node, NodeId, NodeKind, TextAlignH, TextAutoResize, TextCase, TextData, TextDecoration, TextStyle};

use super::css::{DEFAULT_FONT_SIZE, Style, get, length, read};
use super::{Placement, Report, Warnings, baseline, capped, fonts};
use crate::model::TextRun;

const NAME_CHARS: usize = 64;
const REGULAR: u16 = 400;
const NATURAL_LINE_HEIGHT: f64 = 1.2;
const WRAPPED_AFTER_LINES: f64 = 1.5;

pub(super) fn layer(id: NodeId, run: &TextRun, placement: &Placement, warnings: &mut Warnings) -> Node {
    let name: String = run.text.chars().take(NAME_CHARS).collect();
    let mut report = Report::new(&name, warnings);
    let style = &run.style;
    let mut data = TextData::new(String::new());
    data.style = text_style(style, &mut report);
    let case = read(style, "textTransform", &mut report, text_case).unwrap_or_default();
    data.characters = case.apply(&run.text);
    if let Some(align) = read(style, "textAlign", &mut report, text_align) {
        data.align_h = align;
    }
    match &run.line_box {
        Some(line_box) => data.style.line_height = LineHeight::Pixels(line_box.height),
        None if run.r#box.height > WRAPPED_AFTER_LINES * data.style.line_height.resolve(data.style.font_size, NATURAL_LINE_HEIGHT) => {
            data.auto_resize = TextAutoResize::Height;
        }
        None => {}
    }
    data.baseline_nudge = baseline::nudge(run, &data);
    data.hug_nudge = baseline::hug_nudge(placement.size().0, &data) as f32;
    data.browser_gamma = true;
    let (width, height) = placement.size();
    let mut node = Node::new(id, NodeKind::Text(data), &name, width, height);
    node.transform = Affine::translate(placement.offset);
    if let Some(color) = read(style, "color", &mut report, Color::parse_css) {
        node.fills = if color.a > 0.0 { vec![Paint::solid(color)] } else { Vec::new() };
    }
    node
}

fn text_style(style: &Style, report: &mut Report) -> TextStyle {
    let mut text = TextStyle::default();
    if let Some(size) = read(style, "fontSize", report, |value| length(value, DEFAULT_FONT_SIZE, None).filter(|px| *px > 0.0)) {
        text.font_size = size;
    }
    let weight = read(style, "fontWeight", report, font_weight).unwrap_or(REGULAR);
    let italic = read(style, "fontStyle", report, is_italic).unwrap_or(false);
    let face = family_of(style, report);
    if let Some(face) = &face {
        text.font_family.clone_from(&face.family);
    }
    text.font_style = face
        .and_then(|face| face.style.or_else(|| face.installed.then(|| fonts::installed_style(&face.family, weight, italic)).flatten()))
        .unwrap_or_else(|| style_name(weight, italic));
    let size = text.font_size;
    if let Some(height) = read(style, "lineHeight", report, |value| line_height(value, size)) {
        text.line_height = height;
    }
    if let Some(spacing) = read(style, "letterSpacing", report, |value| letter_spacing(value, size)) {
        text.letter_spacing = spacing;
    }
    if let Some(decoration) = read(style, "textDecorationLine", report, text_decoration) {
        text.decoration = decoration;
    }
    text
}

fn text_decoration(value: &str) -> Option<TextDecoration> {
    match value.to_ascii_lowercase().split_whitespace().next()? {
        "none" => Some(TextDecoration::None),
        "underline" => Some(TextDecoration::Underline),
        "line-through" => Some(TextDecoration::Strikethrough),
        _ => None,
    }
}

struct Face {
    family: String,
    installed: bool,
    style: Option<String>,
}

/// The face the text is drawn in; `style` is set when the platform named the face itself.
fn family_of(style: &Style, report: &mut Report) -> Option<Face> {
    let first = read(style, "fontFamily", report, first_family);
    if let Some(platform) = get(style, "platformFont") {
        let family = capped(platform);
        let installed = fonts::is_installed(&family);
        if !installed && let Some((base, face_style)) = fonts::face_of_platform_name(platform) {
            return Some(Face { family: base, installed: true, style: Some(face_style) });
        }
        if !installed {
            report.note(&format!("font {family} is not installed: it is drawn with a fallback until it is"));
        }
        return Some(Face { family, installed, style: None });
    }
    if let Some(family) = get(style, "fontFamily").and_then(fonts::installed_family) {
        return Some(Face { family, installed: true, style: None });
    }
    first.map(|family| Face { family, installed: false, style: None })
}

pub(super) fn first_family(stack: &str) -> Option<String> {
    fonts::stack_families(stack).first().map(|family| capped(family))
}

pub(super) fn font_weight(value: &str) -> Option<u16> {
    match value.to_ascii_lowercase().as_str() {
        "normal" => Some(REGULAR),
        "bold" => Some(700),
        number => number.parse::<f64>().ok().filter(|weight| (1.0..=1000.0).contains(weight)).map(|weight| weight.round() as u16),
    }
}

fn is_italic(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().split_whitespace().next()? {
        "normal" => Some(false),
        "italic" | "oblique" => Some(true),
        _ => None,
    }
}

fn style_name(weight: u16, italic: bool) -> String {
    let name = match (weight + 50) / 100 {
        0 | 1 => "Thin",
        2 => "Extra Light",
        3 => "Light",
        4 => "Regular",
        5 => "Medium",
        6 => "Semi Bold",
        7 => "Bold",
        8 => "Extra Bold",
        _ => "Black",
    };
    match (name, italic) {
        ("Regular", true) => "Italic".to_owned(),
        (name, true) => format!("{name} Italic"),
        (name, false) => name.to_owned(),
    }
}

fn line_height(value: &str, font_size: f64) -> Option<LineHeight> {
    if value.eq_ignore_ascii_case("normal") {
        return Some(LineHeight::Auto);
    }
    if let Ok(multiple) = value.parse::<f64>() {
        return (multiple.is_finite() && multiple >= 0.0).then_some(LineHeight::Percent(multiple * 100.0));
    }
    length(value, font_size, Some(font_size)).filter(|px| *px >= 0.0).map(LineHeight::Pixels)
}

fn letter_spacing(value: &str, font_size: f64) -> Option<LetterSpacing> {
    if value.eq_ignore_ascii_case("normal") {
        return Some(LetterSpacing::default());
    }
    length(value, font_size, None).map(LetterSpacing::Pixels)
}

fn text_case(value: &str) -> Option<TextCase> {
    match value.to_ascii_lowercase().as_str() {
        "none" => Some(TextCase::Original),
        "uppercase" => Some(TextCase::Upper),
        "lowercase" => Some(TextCase::Lower),
        "capitalize" => Some(TextCase::Title),
        _ => None,
    }
}

fn text_align(value: &str) -> Option<TextAlignH> {
    match value.to_ascii_lowercase().as_str() {
        "left" | "start" => Some(TextAlignH::Left),
        "center" => Some(TextAlignH::Center),
        "right" | "end" => Some(TextAlignH::Right),
        "justify" => Some(TextAlignH::Justified),
        _ => None,
    }
}
