//! Readers for the computed CSS values a snapshot carries: lengths, numbers and the splitting of
//! lists. A reader that refuses a value gets it reported by [`read`], never replaced by a guess.

use std::collections::BTreeMap;

use super::Report;

pub(super) type Style = BTreeMap<String, String>;

pub(super) const DEFAULT_FONT_SIZE: f64 = 16.0;

pub(super) fn get<'a>(style: &'a Style, key: &str) -> Option<&'a str> {
    style.get(key).map(|value| value.trim()).filter(|value| !value.is_empty())
}

pub(super) fn keyword(style: &Style, key: &str) -> Option<String> {
    get(style, key).map(str::to_ascii_lowercase)
}

pub(super) fn read<T>(style: &Style, key: &str, report: &mut Report, parse: impl FnOnce(&str) -> Option<T>) -> Option<T> {
    let value = get(style, key)?;
    let parsed = parse(value);
    if parsed.is_none() {
        report.unreadable(key, value);
    }
    parsed
}

pub(super) fn is_hidden(style: &Style) -> bool {
    matches!(keyword(style, "display").as_deref(), Some("none")) || matches!(keyword(style, "visibility").as_deref(), Some("hidden" | "collapse"))
}

pub(super) fn is_contents(style: &Style) -> bool {
    matches!(keyword(style, "display").as_deref(), Some("contents"))
}

pub(super) fn font_size_px(style: &Style) -> f64 {
    get(style, "fontSize").and_then(|value| length(value, DEFAULT_FONT_SIZE, None)).filter(|px| *px > 0.0).unwrap_or(DEFAULT_FONT_SIZE)
}

pub(super) fn length(value: &str, em_px: f64, percent_of: Option<f64>) -> Option<f64> {
    let (number, unit) = ["px", "em", "%"].into_iter().find_map(|unit| value.strip_suffix(unit).map(|number| (number, unit))).unwrap_or((value, ""));
    let number: f64 = number.parse().ok()?;
    let px = match unit {
        "px" => number,
        "em" => number * em_px,
        "%" => number / 100.0 * percent_of?,
        _ if number == 0.0 => 0.0,
        _ => return None,
    };
    px.is_finite().then_some(px)
}

pub(super) fn number_or_percent(value: &str) -> Option<f64> {
    let number = match value.strip_suffix('%') {
        Some(percent) => percent.parse::<f64>().ok()? / 100.0,
        None => value.parse().ok()?,
    };
    number.is_finite().then_some(number)
}

pub(super) fn words(input: &str) -> Vec<&str> {
    split_outside(input, char::is_whitespace)
}

pub(super) fn split_outside(input: &str, separator: impl Fn(char) -> bool) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut quote, mut start) = (0usize, None, 0);
    for (at, c) in input.char_indices() {
        match (quote, c) {
            (Some(open), _) if c == open => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth = depth.saturating_sub(1),
            (None, _) if depth == 0 && separator(c) => {
                parts.extend(input.get(start..at).map(str::trim).filter(|part| !part.is_empty()));
                start = at + c.len_utf8();
            }
            (None, _) => {}
        }
    }
    parts.extend(input.get(start..).map(str::trim).filter(|part| !part.is_empty()));
    parts
}
