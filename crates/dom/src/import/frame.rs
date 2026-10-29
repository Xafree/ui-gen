//! An element becomes a frame: its background, borders, corner radii, shadows, clipping and opacity.

use figcraft_color::{Color, Paint};
use figcraft_doc::geom::Affine;
use figcraft_doc::{Effect, EffectKind, Node, NodeId, NodeKind, Shadow, StrokeAlign};

use super::css::{Style, font_size_px, get, keyword, length, number_or_percent, read, words};
use super::shadow::shadows;
use super::{Placement, Report, Warnings, capped};
use crate::model::Element;

const SIDES: [&str; 4] = ["Top", "Right", "Bottom", "Left"];
const CORNERS: [&str; 4] = ["borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius"];

pub(super) fn build(id: NodeId, element: &Element, placement: &Placement, warnings: &mut Warnings) -> Node {
    let name = capped(element.label.as_deref().filter(|label| !label.is_empty()).unwrap_or(&element.tag));
    let (width, height) = placement.size();
    let mut node = Node::new(id, NodeKind::Frame, &name, width, height);
    node.transform = Affine::translate(placement.offset);
    node.fills.clear();
    node.clip = false;
    let mut report = Report::new(&name, warnings);
    let style = &element.style;
    let em_px = font_size_px(style);
    background(&mut node, style, &mut report);
    border(&mut node, style, em_px, &mut report);
    corners(&mut node, style, em_px, &mut report);
    if let Some(list) = get(style, "boxShadow") {
        node.effects = shadows(list, em_px, &mut report).into_iter().map(effect).collect();
    }
    if let Some(clipped) = read(style, "overflow", &mut report, clips) {
        node.clip = clipped;
    }
    if let Some(opacity) = read(style, "opacity", &mut report, number_or_percent) {
        node.opacity = opacity.clamp(0.0, 1.0) as f32;
    }
    node
}

fn effect((inset, shadow): (bool, Shadow)) -> Effect {
    let kind = if inset { EffectKind::InnerShadow(shadow) } else { EffectKind::DropShadow(shadow) };
    Effect { kind, visible: true }
}

fn background(node: &mut Node, style: &Style, report: &mut Report) {
    if let Some(color) = read(style, "backgroundColor", report, Color::parse_css).filter(|color| color.a > 0.0) {
        node.fills = vec![Paint::solid(color)];
    }
    if let Some(image) = get(style, "backgroundImage").filter(|image| !image.eq_ignore_ascii_case("none")) {
        report.unreadable("backgroundImage", image);
    }
}

/// A stroke has one colour: a side of another colour is left out and reported.
fn border(node: &mut Node, style: &Style, em_px: f64, report: &mut Report) {
    let mut color: Option<Color> = None;
    let mut widths = [0.0; 4];
    for (width, side) in widths.iter_mut().zip(SIDES) {
        let Some((side_width, side_color)) = border_side(style, side, em_px, report) else { continue };
        if *color.get_or_insert(side_color) == side_color {
            *width = side_width;
        } else {
            report.unreadable(&format!("border{side}Color"), get(style, &format!("border{side}Color")).unwrap_or_default());
        }
    }
    let Some(color) = color else { return };
    let [first, ..] = widths;
    node.strokes = vec![Paint::solid(color)];
    node.stroke.align = StrokeAlign::Inside;
    node.stroke.weight = widths.into_iter().fold(0.0, f64::max);
    node.stroke.sides = (!widths.iter().all(|width| *width == first)).then_some(widths);
}

fn border_side(style: &Style, side: &str, em_px: f64, report: &mut Report) -> Option<(f64, Color)> {
    let key = |part: &str| format!("border{side}{part}");
    match keyword(style, &key("Style")).as_deref() {
        None | Some("none" | "hidden") => return None,
        Some("solid") => {}
        Some(_) => {
            report.unreadable(&key("Style"), get(style, &key("Style")).unwrap_or_default());
            return None;
        }
    }
    let width = read(style, &key("Width"), report, |value| length(value, em_px, None).filter(|width| *width >= 0.0))?;
    let color = read(style, &key("Color"), report, Color::parse_css)?;
    (width > 0.0 && color.a > 0.0).then_some((width, color))
}

fn corners(node: &mut Node, style: &Style, em_px: f64, report: &mut Report) {
    let (width, height) = (node.width, node.height);
    let radii = CORNERS.map(|key| match read(style, key, report, |value| radius(value, em_px, width, height)) {
        Some((px, exact)) => {
            if !exact {
                report.unreadable(key, get(style, key).unwrap_or_default());
            }
            px
        }
        None => 0.0,
    });
    node.corners.radii = fit_radii(radii, width, height);
}

/// Layers have circular corners only: an elliptical radius comes out as its smaller semi-axis and is reported.
fn radius(value: &str, em_px: f64, width: f64, height: f64) -> Option<(f64, bool)> {
    let (horizontal, vertical) = match words(value).as_slice() {
        [one] => (length(one, em_px, Some(width))?, length(one, em_px, Some(height))?),
        [horizontal, vertical] => (length(horizontal, em_px, Some(width))?, length(vertical, em_px, Some(height))?),
        _ => return None,
    };
    if horizontal < 0.0 || vertical < 0.0 {
        return None;
    }
    Some((horizontal.min(vertical), (horizontal - vertical).abs() <= 1e-6))
}

fn fit_radii([top_left, top_right, bottom_right, bottom_left]: [f64; 4], width: f64, height: f64) -> [f64; 4] {
    let factor = |side: f64, sum: f64| if sum > side { side / sum } else { 1.0 };
    let scale = factor(width, top_left + top_right)
        .min(factor(width, bottom_left + bottom_right))
        .min(factor(height, top_left + bottom_left))
        .min(factor(height, top_right + bottom_right));
    [top_left, top_right, bottom_right, bottom_left].map(|radius| radius * scale)
}

fn clips(value: &str) -> Option<bool> {
    let keywords = words(value);
    if keywords.is_empty() || keywords.len() > 2 {
        return None;
    }
    let mut clipped = false;
    for keyword in keywords {
        match keyword.to_ascii_lowercase().as_str() {
            "visible" => {}
            "hidden" | "clip" | "scroll" | "auto" => clipped = true,
            _ => return None,
        }
    }
    Some(clipped)
}
