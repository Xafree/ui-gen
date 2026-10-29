//! CSS motion: the transition an element declares between states, and the effects (ripple,
//! spinner) a component draws itself. What does not fit is a note, never an invented value.

use std::collections::BTreeMap;

use figcraft_doc::proto::Easing;
use figcraft_doc::{Document, NodeId};

use super::css::{Style, get, split_outside};
use super::translate::{Effect, LayerFacts, TransitionFact};
use super::{Report, Warnings, cleanup, notes, shortened};
use crate::model::{DomAnimation, Element, SvgNode};

const EFFECT_KEY: &str = "uigen.effect";
const MAX_MS: f64 = 600_000.0;

/// The design curve of a CSS timing function; `None` for steps, `linear()` and anything unreadable.
pub fn css_easing(value: &str) -> Option<Easing> {
    let value = value.trim().to_ascii_lowercase();
    let curve = |x1, y1, x2, y2| Some(Easing::CustomBezier { x1, y1, x2, y2 });
    match value.as_str() {
        "linear" => Some(Easing::Linear),
        "ease" => curve(0.25, 0.1, 0.25, 1.0),
        "ease-in" => curve(0.42, 0.0, 1.0, 1.0),
        "ease-out" => curve(0.0, 0.0, 0.58, 1.0),
        "ease-in-out" => curve(0.42, 0.0, 0.58, 1.0),
        _ => cubic_bezier(&value),
    }
}

fn cubic_bezier(value: &str) -> Option<Easing> {
    let arguments = value.strip_prefix("cubic-bezier(")?.strip_suffix(')')?;
    let numbers =
        arguments.split(',').map(|number| number.trim().parse::<f64>().ok().filter(|number| number.is_finite())).collect::<Option<Vec<_>>>()?;
    let &[x1, y1, x2, y2] = numbers.as_slice() else { return None };
    ((0.0..=1.0).contains(&x1) && (0.0..=1.0).contains(&x2)).then_some(Easing::CustomBezier { x1, y1, x2, y2 })
}

pub(super) fn apply(
    doc: &mut Document,
    sources: &BTreeMap<NodeId, &Element>,
    svgs: &BTreeMap<NodeId, &SvgNode>,
    warnings: &mut Warnings,
    layers: &mut [LayerFacts],
) {
    for layer in layers {
        let element = sources.get(&layer.id);
        let svg = svgs.get(&layer.id);
        if element.is_none() && svg.is_none() {
            continue;
        }
        let name = doc.node(layer.id).map(|node| node.name.clone()).unwrap_or_default();
        {
            let mut report = Report::new(&name, warnings);
            if let Some(element) = element {
                layer.transition = transition(&element.style, &mut report);
                layer.effect = effect(element, &mut report);
            } else if let Some(svg) = svg {
                layer.effect = spinner(&svg.animations, &mut report);
            }
        }
        notes::attach(doc, layer.id, warnings.take_fresh());
        if let (Some(effect), Some(node)) = (layer.effect, doc.node_mut(layer.id)) {
            node.plugin_data.insert(EFFECT_KEY.to_owned(), effect_name(effect).to_owned());
        }
    }
}

fn effect_name(effect: Effect) -> &'static str {
    match effect {
        Effect::Ripple => "ripple",
        Effect::Spinner => "spinner",
    }
}

#[derive(PartialEq)]
struct Timing {
    duration_ms: u32,
    delay_ms: u32,
    easing: Easing,
}

struct Declared {
    property: String,
    timing: Timing,
}

fn list<'a>(style: &'a Style, key: &str, default: &'a str) -> Vec<&'a str> {
    split_outside(get(style, key).unwrap_or(default), |c| c == ',')
}

fn cycled<'a>(values: &[&'a str], index: usize, default: &'a str) -> &'a str {
    values.get(index % values.len().max(1)).copied().unwrap_or(default)
}

/// The longest transition of the element; those sharing its timing join its properties, the others become notes.
fn transition(style: &Style, report: &mut Report) -> Option<TransitionFact> {
    let properties = list(style, "transitionProperty", "all");
    let durations = list(style, "transitionDuration", "0s");
    let delays = list(style, "transitionDelay", "0s");
    let curves = list(style, "transitionTimingFunction", "ease");
    let mut declared = Vec::new();
    for (index, property) in properties.iter().enumerate().filter(|(_, property)| !property.eq_ignore_ascii_case("none")) {
        let words = [cycled(&durations, index, "0s"), cycled(&delays, index, "0s"), cycled(&curves, index, "ease")];
        if let Some(timing) = timing(property, words, report) {
            declared.push(Declared { property: (*property).to_owned(), timing });
        }
    }
    let longest =
        declared.iter().map(|entry| &entry.timing).reduce(|best, timing| if timing.duration_ms > best.duration_ms { timing } else { best })?;
    let (kept, left): (Vec<&Declared>, Vec<&Declared>) = declared.iter().partition(|entry| entry.timing == *longest);
    for entry in left {
        report.note(&format!(
            "transition {} {}ms left out: only the longest transition ({}ms) is kept",
            entry.property, entry.timing.duration_ms, longest.duration_ms
        ));
    }
    Some(TransitionFact {
        properties: kept.into_iter().map(|entry| entry.property.clone()).collect(),
        duration_ms: longest.duration_ms,
        delay_ms: longest.delay_ms,
        easing: longest.easing,
    })
}

/// `None` when the transition moves nothing (no duration) or cannot be translated; the second case is reported.
fn timing(property: &str, [duration, delay, curve]: [&str; 3], report: &mut Report) -> Option<Timing> {
    let Some(duration_ms) = time(duration) else {
        report.unreadable("transitionDuration", duration);
        return None;
    };
    if duration_ms == 0 {
        return None;
    }
    let Some(delay_ms) = time(delay) else {
        report.unreadable("transitionDelay", delay);
        return None;
    };
    let Some(easing) = css_easing(curve) else {
        report.note(&format!("transition {property} {}: the curve cannot be translated", shortened(curve)));
        return None;
    };
    Some(Timing { duration_ms, delay_ms, easing })
}

fn time(value: &str) -> Option<u32> {
    let value = value.to_ascii_lowercase();
    let (number, scale) = match value.strip_suffix("ms") {
        Some(number) => (number, 1.0),
        None => (value.strip_suffix('s')?, 1000.0),
    };
    let milliseconds = number.trim().parse::<f64>().ok()? * scale;
    (milliseconds.is_finite() && milliseconds >= 0.0).then(|| milliseconds.round().min(MAX_MS) as u32)
}

fn effect(element: &Element, report: &mut Report) -> Option<Effect> {
    if cleanup::is_ripple(element) {
        return Some(Effect::Ripple);
    }
    spinner(&element.animations, report)
}

fn spinner(animations: &[DomAnimation], report: &mut Report) -> Option<Effect> {
    let mut found = None;
    for animation in animations {
        if found.is_none() && turns_forever(animation) {
            found = Some(Effect::Spinner);
        } else {
            report.unreadable("animation", &animation.name);
        }
    }
    found
}

fn turns_forever(animation: &DomAnimation) -> bool {
    animation.iterations.is_none()
        && animation.properties.iter().any(|property| property.eq_ignore_ascii_case("transform") || property.eq_ignore_ascii_case("rotate"))
}
