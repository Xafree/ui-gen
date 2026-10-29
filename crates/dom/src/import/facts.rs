//! The values a translated layer holds that a design token could stand for, read off the layer
//! itself; `vars` names the CSS variable a value came from, when the page had one.

use std::collections::BTreeMap;

use figcraft_color::{Color, Paint};
use figcraft_doc::{Document, EffectKind, LayoutMode, Node, NodeId, NodeKind};

use super::css::{DEFAULT_FONT_SIZE, get, length};
use super::translate::{LayerFacts, TokenFact, TokenValue};
use crate::model::Element;

const RADII: [(&str, &str); 4] = [
    ("topLeftRadius", "borderTopLeftRadius"),
    ("topRightRadius", "borderTopRightRadius"),
    ("bottomRightRadius", "borderBottomRightRadius"),
    ("bottomLeftRadius", "borderBottomLeftRadius"),
];
const PADDING: [&str; 4] = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
const BORDER_COLORS: [&str; 4] = ["borderTopColor", "borderRightColor", "borderBottomColor", "borderLeftColor"];
const BORDER_WIDTHS: [&str; 4] = ["borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth"];

pub(super) fn fill(doc: &Document, sources: &BTreeMap<NodeId, &Element>, layers: &mut [LayerFacts]) {
    for layer in layers {
        let Some(node) = doc.node(layer.id) else { continue };
        let parent = node.parent.and_then(|parent| sources.get(&parent).copied());
        layer.tokens = match (&node.kind, sources.get(&layer.id)) {
            (NodeKind::Frame, Some(element)) => frame(node, &element.vars),
            (NodeKind::Text(data), _) => text(node, data.style.font_size, parent),
            _ => Vec::new(),
        };
    }
}

struct Tokens<'a> {
    vars: &'a BTreeMap<String, String>,
    facts: Vec<TokenFact>,
}

impl<'a> Tokens<'a> {
    fn new(vars: &'a BTreeMap<String, String>) -> Self {
        Self { vars, facts: Vec::new() }
    }

    fn color(&mut self, field: &str, color: Option<Color>, keys: &[&str]) {
        if let Some(color) = color {
            self.push(field, TokenValue::Color(color), keys);
        }
    }

    fn number(&mut self, field: &str, value: f64, keys: &[&str]) {
        if value.is_finite() && value > 0.0 {
            self.push(field, TokenValue::Number(value), keys);
        }
    }

    fn push(&mut self, field: &str, value: TokenValue, keys: &[&str]) {
        let exposed = keys.iter().find_map(|key| self.vars.get(*key)).cloned();
        self.facts.push(TokenFact { field: field.to_owned(), value, exposed });
    }
}

fn first_color(paints: &[Paint]) -> Option<Color> {
    paints.first().filter(|paint| paint.visible).and_then(Paint::solid_color)
}

fn frame(node: &Node, vars: &BTreeMap<String, String>) -> Vec<TokenFact> {
    let mut tokens = Tokens::new(vars);
    tokens.color("fill", first_color(&node.fills), &["backgroundColor"]);
    let stroke = first_color(&node.strokes);
    tokens.color("stroke", stroke, &BORDER_COLORS);
    if stroke.is_some() && node.stroke.sides.is_none() {
        tokens.number("strokeWeight", node.stroke.weight, &BORDER_WIDTHS);
    }
    for ((field, key), radius) in RADII.into_iter().zip(node.corners.radii) {
        tokens.number(field, radius, &[key]);
    }
    if let Some(layout) = &node.layout {
        for (key, padding) in PADDING.into_iter().zip(layout.padding) {
            tokens.number(key, padding, &[key]);
        }
        let gap_key = if layout.mode == LayoutMode::Vertical { "rowGap" } else { "columnGap" };
        tokens.number("gap", layout.gap, &[gap_key]);
    }
    if node.opacity < 1.0 {
        tokens.number("opacity", f64::from(node.opacity), &["opacity"]);
    }
    for (index, effect) in node.effects.iter().enumerate() {
        if let EffectKind::DropShadow(shadow) | EffectKind::InnerShadow(shadow) = &effect.kind {
            tokens.color(&format!("effects/{index}/color"), Some(shadow.color), &[]);
        }
    }
    tokens.facts
}

/// A text run has no variables of its own: it exposes those its parent element declared, when the parent computed the same value.
fn text(node: &Node, font_size: f64, parent: Option<&Element>) -> Vec<TokenFact> {
    let color = first_color(&node.fills);
    let inherited: BTreeMap<String, String> = parent
        .map(|element| {
            let same_color = color.is_some() && get(&element.style, "color").and_then(Color::parse_css) == color;
            let same_size = get(&element.style, "fontSize").and_then(|value| length(value, DEFAULT_FONT_SIZE, None)) == Some(font_size);
            let keeps = |key: &str| (key == "color" && same_color) || (key == "fontSize" && same_size);
            element.vars.iter().filter(|(key, _)| keeps(key)).map(|(key, name)| (key.clone(), name.clone())).collect()
        })
        .unwrap_or_default();
    let mut tokens = Tokens::new(&inherited);
    tokens.color("fill", color, &["color"]);
    tokens.number("fontSize", font_size, &["fontSize"]);
    tokens.facts
}
