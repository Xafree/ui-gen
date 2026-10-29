//! A text run becomes a text layer at its box, in the font its parent element computed.

use figcraft_color::{Color, Paint};
use figcraft_doc::geom::Affine;
use figcraft_doc::{LetterSpacing, LineHeight, Node, NodeId, NodeKind, TextAlignH, TextAutoResize, TextCase, TextData, TextStyle};

use super::css::{DEFAULT_FONT_SIZE, Style, length, read, split_outside};
use super::{Report, Warnings, capped};
use crate::model::TextRun;

const NAME_CHARS: usize = 64;
const REGULAR: u16 = 400;
const NATURAL_LINE_HEIGHT: f64 = 1.2;
const WRAPPED_AFTER_LINES: f64 = 1.5;

pub(super) fn layer(id: NodeId, run: &TextRun, offset: (f64, f64), warnings: &mut Warnings) -> Node {
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
    if run.r#box.height > WRAPPED_AFTER_LINES * data.style.line_height.resolve(data.style.font_size, NATURAL_LINE_HEIGHT) {
        data.auto_resize = TextAutoResize::Height;
    }
    let mut node = Node::new(id, NodeKind::Text(data), &name, run.r#box.width, run.r#box.height);
    node.transform = Affine::translate(offset);
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
    if let Some(family) = read(style, "fontFamily", report, first_family) {
        text.font_family = family;
    }
    let weight = read(style, "fontWeight", report, font_weight).unwrap_or(REGULAR);
    let italic = read(style, "fontStyle", report, is_italic).unwrap_or(false);
    text.font_style = style_name(weight, italic);
    let size = text.font_size;
    if let Some(height) = read(style, "lineHeight", report, |value| line_height(value, size)) {
        text.line_height = height;
    }
    if let Some(spacing) = read(style, "letterSpacing", report, |value| letter_spacing(value, size)) {
        text.letter_spacing = spacing;
    }
    text
}

fn first_family(stack: &str) -> Option<String> {
    let first = split_outside(stack, |c| c == ',').into_iter().next()?;
    let family = first.trim_matches(|c: char| matches!(c, '"' | '\'') || c.is_whitespace());
    (!family.is_empty()).then(|| capped(family))
}

fn font_weight(value: &str) -> Option<u16> {
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
