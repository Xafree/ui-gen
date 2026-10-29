//! `box-shadow` lists: `[inset] <x> <y> [<blur> [<spread>]] [<colour>]`, the colour first or last
//! (Chromium writes it first and `inset` last).

use figcraft_color::Color;
use figcraft_doc::Shadow;

use super::Report;
use super::css::{length, split_outside, words};

/// In painting order: CSS puts the first entry on top, a layer list the last.
pub(super) fn shadows(value: &str, em_px: f64, report: &mut Report) -> Vec<(bool, Shadow)> {
    if value.eq_ignore_ascii_case("none") {
        return Vec::new();
    }
    let mut entries: Vec<(bool, Shadow)> = split_outside(value, |c| c == ',')
        .into_iter()
        .filter_map(|entry| {
            let parsed = shadow(entry, em_px);
            if parsed.is_none() {
                report.unreadable("boxShadow", entry);
            }
            parsed
        })
        .filter(|(_, shadow)| shadow.color.a > 0.0)
        .collect();
    entries.reverse();
    entries
}

fn shadow(entry: &str, em_px: f64) -> Option<(bool, Shadow)> {
    let mut inset = false;
    let mut color = None;
    let mut lengths: Vec<f64> = Vec::new();
    let mut run_ended = false;
    for word in words(entry) {
        if word.eq_ignore_ascii_case("inset") {
            if std::mem::replace(&mut inset, true) {
                return None;
            }
            run_ended = !lengths.is_empty();
        } else if starts_like_a_number(word) {
            if run_ended {
                return None;
            }
            lengths.push(length(word, em_px, None)?);
        } else {
            if color.replace(Color::parse_css(word)?).is_some() {
                return None;
            }
            run_ended = !lengths.is_empty();
        }
    }
    let [x, y, rest @ ..] = lengths.as_slice() else { return None };
    let (radius, spread) = match rest {
        [] => (0.0, 0.0),
        [radius] => (*radius, 0.0),
        [radius, spread] => (*radius, *spread),
        _ => return None,
    };
    if radius < 0.0 {
        return None;
    }
    let shadow = Shadow { color: color.unwrap_or(Color::BLACK), offset: (*x, *y), radius, spread, ..Shadow::default() };
    Some((inset, shadow))
}

fn starts_like_a_number(word: &str) -> bool {
    word.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '+' | '-' | '.'))
}
