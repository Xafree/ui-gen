//! The faces the text runs of a snapshot are drawn in, in the order the import makes their layers.

use super::capped;
use super::css::{DEFAULT_FONT_SIZE, Style, get, is_hidden, length};
use super::text::{first_family, font_weight};
use crate::model::{DomBox, DomNode, Snapshot, TextRun};

const REGULAR: u16 = 400;

#[derive(Debug, Clone, PartialEq)]
pub struct TextFace {
    pub text_box: DomBox,
    pub line_box: Option<DomBox>,
    pub family: String,
    pub size: f64,
    pub weight: u16,
}

/// Every text run the import turns into a layer: hidden elements and their subtrees left out. The
/// family is the one the platform drew with, else the first of the font stack.
pub fn text_faces(snapshot: &Snapshot) -> Vec<TextFace> {
    let mut faces = Vec::new();
    let mut pending = vec![&snapshot.root];
    while let Some(node) = pending.pop() {
        match node {
            DomNode::Element(element) if !is_hidden(&element.style) => pending.extend(element.children.iter().rev()),
            DomNode::Text(run) => faces.push(face_of(run)),
            DomNode::Element(_) | DomNode::Svg(_) => {}
        }
    }
    faces
}

fn face_of(run: &TextRun) -> TextFace {
    let style: &Style = &run.style;
    let family = get(style, "platformFont").map(capped).or_else(|| get(style, "fontFamily").and_then(first_family)).unwrap_or_default();
    let size = get(style, "fontSize").and_then(|value| length(value, DEFAULT_FONT_SIZE, None)).filter(|px| *px > 0.0).unwrap_or(DEFAULT_FONT_SIZE);
    TextFace { text_box: run.r#box, line_box: run.line_box, family, size, weight: weight_drawn(style) }
}

const WEIGHT_NAMES: [(&str, u16); 12] = [
    ("thin", 100),
    ("extralight", 200),
    ("ultralight", 200),
    ("light", 300),
    ("medium", 500),
    ("semibold", 600),
    ("demibold", 600),
    ("extrabold", 800),
    ("ultrabold", 800),
    ("black", 900),
    ("bold", 700),
    ("heavy", 900),
];

fn named_weight(name: &str) -> Option<u16> {
    let squashed: String = name.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect();
    WEIGHT_NAMES.iter().find(|(word, _)| squashed.contains(word)).map(|(_, weight)| *weight)
}

fn post_script_style(post_script: &str) -> &str {
    let style = post_script.split_once('-').map_or("", |(_, style)| style);
    style.strip_suffix("MT").unwrap_or(style)
}

fn weight_drawn(style: &Style) -> u16 {
    let face = get(style, "platformFontPostScript").map(post_script_style);
    if face.is_some_and(|face| face.is_empty() || face.eq_ignore_ascii_case("regular") || face.eq_ignore_ascii_case("italic")) {
        return REGULAR;
    }
    face.and_then(named_weight)
        .or_else(|| get(style, "platformFont").and_then(named_weight))
        .or_else(|| get(style, "fontWeight").and_then(font_weight))
        .unwrap_or(REGULAR)
}
