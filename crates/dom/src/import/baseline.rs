//! Where the browser put a line of text: its half leading is floored and its baseline falls on a whole pixel.

use figcraft_doc::TextData;

use crate::model::{DomBox, TextRun};

const MAX_HUG_NUDGE: f64 = 1.0;

/// The line box the browser laid out, when the snapshot measured the baseline; an older snapshot keeps its estimate.
pub(super) fn line_box(run: &TextRun) -> Option<DomBox> {
    let line = run.line_box?;
    if run.baseline.is_none() {
        return Some(line);
    }
    let half_leading = ((line.height - run.r#box.height) / 2.0).floor();
    Some(DomBox { y: run.r#box.y - half_leading, ..line })
}

/// What `data`, its style final, must add to its natural baseline to draw on the browser's; nothing for a baseline out of reason.
pub(super) fn nudge(run: &TextRun, data: &TextData) -> f64 {
    let Some(baseline) = run.baseline else { return 0.0 };
    let top = line_box(run).map_or(run.r#box.y, |line| line.y);
    let Some(natural) = figcraft_text::layout_cached(data, None, None).lines.first().map(|line| line.baseline) else { return 0.0 };
    let wanted = baseline.round() - top.round() - natural;
    if wanted.is_finite() && wanted.abs() <= data.style.font_size { wanted } else { 0.0 }
}

/// What `data`, its style final, must add to its natural width to hug the `width` the browser snapped its box to; nothing for a width out of reason.
pub(super) fn hug_nudge(width: f64, data: &TextData) -> f64 {
    let wanted = width - figcraft_text::layout_cached(data, None, None).width;
    if wanted.is_finite() && wanted.abs() <= MAX_HUG_NUDGE { wanted } else { 0.0 }
}
