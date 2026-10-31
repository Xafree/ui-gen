//! `background-image` lists: gradients become paints, what no paint can hold is reported.

use figcraft_color::{Color, ColorStop, Gradient, GradientKind, Paint, PaintKind};

use super::Report;
use super::css::{Style, get, length, split_outside, words};

const SIDE_ANGLES: [(&str, f64); 4] = [("to top", 0.0), ("to right", 90.0), ("to bottom", 180.0), ("to left", 270.0)];
const CORNER_DIRECTIONS: [(&str, f64, f64); 8] = [
    ("to top right", 1.0, 1.0),
    ("to right top", 1.0, 1.0),
    ("to bottom right", 1.0, -1.0),
    ("to right bottom", 1.0, -1.0),
    ("to bottom left", -1.0, -1.0),
    ("to left bottom", -1.0, -1.0),
    ("to top left", -1.0, 1.0),
    ("to left top", -1.0, 1.0),
];
const DEFAULT_LAYOUT: [(&str, &[&str]); 3] = [
    ("backgroundSize", &["auto", "auto auto", "100%", "100% 100%"]),
    ("backgroundPosition", &["0% 0%", "0px 0px"]),
    ("backgroundClip", &["border-box"]),
];

/// The gradients of `background-image`, in painting order: CSS puts the first entry on top, a fill list the last.
pub(super) fn background_images(style: &Style, (width, height): (f64, f64), report: &mut Report) -> Vec<Paint> {
    let Some(list) = get(style, "backgroundImage").filter(|list| !list.eq_ignore_ascii_case("none")) else { return Vec::new() };
    report_custom_layout(style, report);
    let mut paints: Vec<Paint> = split_outside(list, |c| c == ',')
        .into_iter()
        .filter_map(|image| {
            let gradient = gradient(image, width, height);
            if gradient.is_none() {
                report.unreadable("backgroundImage", image);
            }
            gradient.map(gradient_paint)
        })
        .collect();
    paints.reverse();
    paints
}

fn report_custom_layout(style: &Style, report: &mut Report) {
    for (key, defaults) in DEFAULT_LAYOUT {
        let Some(value) = get(style, key) else { continue };
        let all_default = split_outside(value, |c| c == ',').iter().all(|part| defaults.iter().any(|default| part.eq_ignore_ascii_case(default)));
        if !all_default {
            report.unreadable(key, value);
        }
    }
}

fn gradient_paint(gradient: Gradient) -> Paint {
    Paint { kind: PaintKind::Gradient(gradient), opacity: 1.0, visible: true, blend: Default::default(), color_alpha: 1.0 }
}

fn gradient(image: &str, width: f64, height: f64) -> Option<Gradient> {
    if !(width > 0.0 && height > 0.0) {
        return None;
    }
    let image = image.trim();
    if let Some(inner) = image.strip_prefix("linear-gradient(").and_then(|rest| rest.strip_suffix(')')) {
        return linear(inner, width, height);
    }
    radial(image.strip_prefix("radial-gradient(")?.strip_suffix(')')?, width, height)
}

/// A corner keyword aims so that the middle of the gradient runs through the two other corners, whatever the shape of the box.
fn angle_of(word: &str, width: f64, height: f64) -> Option<f64> {
    if let Some((_, degrees)) = SIDE_ANGLES.iter().find(|(side, _)| *side == word) {
        return Some(*degrees);
    }
    if let Some((_, right, up)) = CORNER_DIRECTIONS.iter().find(|(corner, ..)| *corner == word) {
        return Some((right * height).atan2(up * width).to_degrees());
    }
    let number = |suffix: &str| word.strip_suffix(suffix).and_then(|value| value.parse::<f64>().ok());
    number("deg")
        .or_else(|| number("turn").map(|turns| turns * 360.0))
        .or_else(|| number("rad").map(f64::to_degrees))
        .filter(|angle| angle.is_finite())
}

/// The CSS gradient line: through the centre, long enough for its ends to meet the corners.
fn linear(inner: &str, width: f64, height: f64) -> Option<Gradient> {
    let parts = split_outside(inner, |c| c == ',');
    let (first, rest) = parts.split_first()?;
    let (degrees, stops) = match angle_of(first, width, height) {
        Some(degrees) => (degrees, rest),
        None => (180.0, parts.as_slice()),
    };
    let (sin, cos) = degrees.to_radians().sin_cos();
    let line = (width * sin).abs() + (height * cos).abs();
    if line <= 0.0 {
        return None;
    }
    let (half_x, half_y) = (sin * line / 2.0, -cos * line / 2.0);
    let (center_x, center_y) = (width / 2.0, height / 2.0);
    let unit = |x: f64, y: f64| (x / width, y / height);
    Some(Gradient {
        kind: GradientKind::Linear,
        stops: color_stops(stops, line)?,
        from: unit(center_x - half_x, center_y - half_y),
        to: unit(center_x + half_x, center_y + half_y),
        width: unit(center_x - half_x + half_y, center_y - half_y - half_x),
    })
}

/// The default shape only, centred: a farthest-corner ellipse, or a circle.
fn radial(inner: &str, width: f64, height: f64) -> Option<Gradient> {
    let parts = split_outside(inner, |c| c == ',');
    let (first, rest) = parts.split_first()?;
    let (shape, stops) = if Color::parse_css(words(first).first()?).is_some() { ("ellipse", parts.as_slice()) } else { (*first, rest) };
    let (x_radius, y_radius) = match shape {
        "ellipse" | "ellipse at 50% 50%" | "at 50% 50%" | "at center" | "ellipse at center" => {
            (width / 2.0 * std::f64::consts::SQRT_2, height / 2.0 * std::f64::consts::SQRT_2)
        }
        "circle" | "circle at 50% 50%" | "circle at center" => {
            let radius = width.hypot(height) / 2.0;
            (radius, radius)
        }
        _ => return None,
    };
    Some(Gradient {
        kind: GradientKind::Radial,
        stops: color_stops(stops, x_radius)?,
        from: (0.5, 0.5),
        to: (0.5 + x_radius / width, 0.5),
        width: (0.5, 0.5 + y_radius / height),
    })
}

/// A stop without a position sits evenly between its neighbours; a stop with two positions counts twice.
fn color_stops(parts: &[&str], line: f64) -> Option<Vec<ColorStop>> {
    let mut stops: Vec<(Color, Option<f64>)> = Vec::new();
    for part in parts {
        let items = words(part);
        let (color, positions) = items.split_first()?;
        let color = Color::parse_css(color)?;
        match positions {
            [] => stops.push((color, None)),
            [_] | [_, _] => {
                for position in positions {
                    stops.push((color, Some(length(position, 16.0, Some(line))? / line)));
                }
            }
            _ => return None,
        }
    }
    if stops.len() < 2 {
        return None;
    }
    let last = stops.len() - 1;
    let known: Vec<(usize, f64)> = stops
        .iter()
        .enumerate()
        .filter_map(|(at, (_, position))| match (at, position) {
            (_, Some(position)) => Some((at, *position)),
            (0, None) => Some((0, 0.0)),
            (at, None) if at == last => Some((at, 1.0)),
            _ => None,
        })
        .collect();
    let position_at = |at: usize| {
        let before = known.iter().rev().find(|(index, _)| *index <= at)?;
        let after = known.iter().find(|(index, _)| *index >= at)?;
        let span = (after.0 - before.0).max(1) as f64;
        Some(before.1 + (after.1 - before.1) * (at - before.0) as f64 / span)
    };
    stops.iter().enumerate().map(|(at, (color, _))| Some(ColorStop { position: position_at(at)?.clamp(0.0, 1.0) as f32, color: *color })).collect()
}
