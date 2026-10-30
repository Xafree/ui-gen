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
    let weight = get(style, "fontWeight").and_then(font_weight).unwrap_or(REGULAR);
    TextFace { text_box: run.r#box, line_box: run.line_box, family, size, weight }
}
