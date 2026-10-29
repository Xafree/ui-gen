//! The CSS Color 4 forms Chromium writes in computed styles besides `rgb()` and `hsl()`:
//! `oklab()` and `color()` (`srgb`, `srgb-linear`, `display-p3`). Broken or non-finite input is
//! refused, never clamped into a guess.

use crate::lch::Oklch;
use crate::profile::encode;
use crate::{Color, ColorProfile, css_number, parse_alpha};

/// `Some(result)` when `text` (trimmed, lower case) is an `oklab()` or `color()` call, `None` for
/// any other text.
pub(crate) fn modern_function(text: &str) -> Option<Option<Color>> {
    if let Some(inner) = arguments(text, "oklab") {
        return Some(oklab(inner));
    }
    arguments(text, "color").map(color_function)
}

fn arguments<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.strip_prefix(name)?.strip_prefix('(')?.strip_suffix(')')
}

/// Lightness, a and b, then an optional alpha: a percentage of 1 for lightness and of 0.4 for a and b.
fn oklab(inner: &str) -> Option<Color> {
    let (parts, alpha) = components(inner)?;
    let [l, a, b] = parts.as_slice() else { return None };
    let (l, a, b) = (css_number(l, 1.0)?, css_number(a, 0.4)?, css_number(b, 0.4)?);
    Some(Oklch::from_lab(l, a, b).to_color().with_alpha(alpha))
}

/// A predefined RGB space and its three channels (numbers, or percentages of 1), then an optional alpha.
fn color_function(inner: &str) -> Option<Color> {
    let (parts, alpha) = components(inner)?;
    let [space, r, g, b] = parts.as_slice() else { return None };
    let (r, g, b) = (css_number(r, 1.0)?, css_number(g, 1.0)?, css_number(b, 1.0)?);
    match *space {
        "srgb" => Some(Color::new(r, g, b, alpha)),
        "srgb-linear" => Some(Color::new(encode(f64::from(r)), encode(f64::from(g)), encode(f64::from(b)), alpha)),
        "display-p3" => Some(ColorProfile::DisplayP3.conversion_to(ColorProfile::Srgb).apply(Color::new(r, g, b, alpha))),
        _ => None,
    }
}

/// The words before an optional `/ alpha`, and the alpha (1 when it is absent or `none`).
fn components(inner: &str) -> Option<(Vec<&str>, f32)> {
    let (body, alpha) = inner.split_once('/').map_or((inner, "none"), |(body, alpha)| (body, alpha.trim()));
    Some((body.split_whitespace().collect(), alpha_value(alpha)?))
}

fn alpha_value(alpha: &str) -> Option<f32> {
    if alpha == "none" { Some(1.0) } else { parse_alpha(alpha).filter(|v| v.is_finite()) }
}
