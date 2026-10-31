//! What CSS paints beyond fills and strokes: focus outlines, filters, blend modes. An effect without
//! a layer equivalent is named in a note on the layer, never dropped in silence.

use figcraft_color::{BlendMode, Color};
use figcraft_doc::{Blur, Effect, EffectKind, Node, Shadow};

use super::Report;
use super::css::{Style, get, keyword, length, split_outside, words};

const NO_LAYER_EQUIVALENT: [(&str, &str); 3] = [("clipPath", "none"), ("maskImage", "none"), ("backgroundBlendMode", "normal")];
const IDENTITY: [f64; 4] = [1.0, 0.0, 0.0, 1.0];
const SAME: f64 = 1e-3;

pub(super) fn apply(node: &mut Node, style: &Style, em_px: f64, report: &mut Report) {
    outline(node, style, em_px, report);
    filter(node, style, em_px, report);
    backdrop(node, style, em_px, report);
    blend(node, style, report);
    transform(style, report);
    for (key, default) in NO_LAYER_EQUIVALENT {
        if let Some(value) = get(style, key).filter(|value| !value.split(',').all(|part| part.trim().eq_ignore_ascii_case(default))) {
            report.unreadable(key, value);
        }
    }
    if let Some(content) = get(style, "uigenLostPseudo") {
        report.unreadable("pseudo-element", content);
    }
}

/// An outline follows the corners of the box: a ring cast by its outline. A gap between the two cannot be left empty.
fn outline(node: &mut Node, style: &Style, em_px: f64, report: &mut Report) {
    let kind = keyword(style, "outlineStyle");
    if matches!(kind.as_deref(), None | Some("none" | "hidden")) {
        return;
    }
    let width = get(style, "outlineWidth").and_then(|value| length(value, em_px, None)).unwrap_or(0.0);
    let Some(color) = get(style, "outlineColor").and_then(Color::parse_css).filter(|color| color.a > 0.0 && width > 0.0) else { return };
    let offset = get(style, "outlineOffset").and_then(|value| length(value, em_px, None)).unwrap_or(0.0);
    if offset.abs() > SAME {
        report.unreadable("outlineOffset", get(style, "outlineOffset").unwrap_or_default());
    }
    if !matches!(kind.as_deref(), Some("solid" | "auto")) {
        report.unreadable("outlineStyle", get(style, "outlineStyle").unwrap_or_default());
    }
    let shadow = Shadow { color, offset: (0.0, 0.0), radius: 0.0, spread: width + offset.max(0.0), from_outline: true, ..Shadow::default() };
    node.effects.push(Effect { kind: EffectKind::DropShadow(shadow), visible: true });
}

fn functions(value: &str) -> Vec<(String, &str)> {
    words(value)
        .into_iter()
        .filter_map(|word| {
            let (name, rest) = word.split_once('(')?;
            Some((name.to_ascii_lowercase(), rest.strip_suffix(')')?))
        })
        .collect()
}

fn filter(node: &mut Node, style: &Style, em_px: f64, report: &mut Report) {
    let Some(value) = get(style, "filter").filter(|value| !value.eq_ignore_ascii_case("none")) else { return };
    for (name, arguments) in functions(value) {
        let kind = match name.as_str() {
            "blur" => length(arguments, em_px, None).map(|deviation| EffectKind::LayerBlur(blur(deviation))),
            "drop-shadow" => drop_shadow(arguments, em_px).map(EffectKind::DropShadow),
            _ => None,
        };
        match kind {
            Some(kind) => node.effects.push(Effect { kind, visible: true }),
            None => report.unreadable("filter", &format!("{name}({arguments})")),
        }
    }
}

/// The CSS `blur()` argument is a standard deviation, a blur radius twice that.
fn blur(deviation: f64) -> Blur {
    Blur { radius: deviation * 2.0, ..Blur::default() }
}

fn drop_shadow(arguments: &str, em_px: f64) -> Option<Shadow> {
    let mut color = Color::BLACK;
    let mut lengths = Vec::new();
    for word in split_outside(arguments, char::is_whitespace) {
        match length(word, em_px, None) {
            Some(px) => lengths.push(px),
            None => color = Color::parse_css(word)?,
        }
    }
    let [x, y, rest @ ..] = lengths.as_slice() else { return None };
    Some(Shadow { color, offset: (*x, *y), radius: rest.first().copied().unwrap_or(0.0) * 2.0, spread: 0.0, ..Shadow::default() })
}

fn backdrop(node: &mut Node, style: &Style, em_px: f64, report: &mut Report) {
    let Some(value) = get(style, "backdropFilter").filter(|value| !value.eq_ignore_ascii_case("none")) else { return };
    for (name, arguments) in functions(value) {
        match (name.as_str(), length(arguments, em_px, None)) {
            ("blur", Some(deviation)) => node.effects.push(Effect { kind: EffectKind::BackgroundBlur(blur(deviation)), visible: true }),
            _ => report.unreadable("backdropFilter", &format!("{name}({arguments})")),
        }
    }
}

fn blend(node: &mut Node, style: &Style, report: &mut Report) {
    let Some(value) = keyword(style, "mixBlendMode").filter(|value| value != "normal") else { return };
    match blend_mode(&value) {
        Some(mode) => node.blend = mode,
        None => report.unreadable("mixBlendMode", &value),
    }
}

fn blend_mode(name: &str) -> Option<BlendMode> {
    Some(match name {
        "multiply" => BlendMode::Multiply,
        "screen" => BlendMode::Screen,
        "overlay" => BlendMode::Overlay,
        "darken" => BlendMode::Darken,
        "lighten" => BlendMode::Lighten,
        "color-dodge" => BlendMode::ColorDodge,
        "color-burn" => BlendMode::ColorBurn,
        "hard-light" => BlendMode::HardLight,
        "soft-light" => BlendMode::SoftLight,
        "difference" => BlendMode::Difference,
        "exclusion" => BlendMode::Exclusion,
        "hue" => BlendMode::Hue,
        "saturation" => BlendMode::Saturation,
        "color" => BlendMode::Color,
        "luminosity" => BlendMode::Luminosity,
        _ => return None,
    })
}

/// A box already holds a translation; a rotation or a scale comes out as the bounding box of the result.
fn transform(style: &Style, report: &mut Report) {
    if let Some(value) = get(style, "transform").filter(|value| !is_translation(value)) {
        report.unreadable("transform", value);
    }
    for key in ["rotate", "scale"] {
        if let Some(value) = get(style, key).filter(|value| !value.eq_ignore_ascii_case("none")) {
            report.unreadable(key, value);
        }
    }
}

pub(super) fn is_scaled(style: &Style) -> bool {
    get(style, "transform").is_some_and(|value| !is_translation(value))
        || get(style, "scale").is_some_and(|value| !value.eq_ignore_ascii_case("none"))
}

fn is_translation(value: &str) -> bool {
    if value.eq_ignore_ascii_case("none") {
        return true;
    }
    let Some(inner) = value.strip_prefix("matrix(").and_then(|rest| rest.strip_suffix(')')) else { return false };
    let numbers: Vec<f64> = inner.split(',').filter_map(|number| number.trim().parse().ok()).collect();
    numbers.len() == 6 && numbers.iter().zip(IDENTITY).all(|(number, identity)| (number - identity).abs() <= SAME)
}
