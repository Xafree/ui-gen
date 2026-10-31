//! A nudge was measured on one font, size and line: it holds only while they stay.

use crate::{LetterSpacing, LineHeight, StyleRun, TextData};

const BASELINE: f64 = 0.6;
const HUG: f32 = -0.3;

type Change = fn(&mut TextData);

fn nudged() -> TextData {
    let mut text = TextData::new("Button".to_owned());
    text.set_nudges(BASELINE, HUG);
    text
}

fn holds_all(text: &TextData) -> bool {
    text.held_baseline_nudge() == BASELINE && text.held_hug_nudge() == HUG
}

#[test]
fn a_nudge_holds_until_a_metric_changes() {
    assert!(holds_all(&nudged()));
    let changes: [(&str, Change); 7] = [
        ("family", |text| text.style.font_family = "Other".to_owned()),
        ("face", |text| text.style.font_style = "Bold".to_owned()),
        ("size", |text| text.style.font_size += 1.0),
        ("line height", |text| text.style.line_height = LineHeight::Pixels(40.0)),
        ("letter spacing", |text| text.style.letter_spacing = LetterSpacing::Pixels(2.0)),
        ("axis", |text| {
            text.style.variations.insert("wght".to_owned(), 700.0);
        }),
        ("run", |text| {
            let mut style = text.style.clone();
            style.font_size += 5.0;
            text.runs.push(StyleRun { start: 0, end: 2, style });
        }),
    ];
    for (what, change) in changes {
        let mut text = nudged();
        change(&mut text);
        assert_eq!((text.held_baseline_nudge(), text.held_hug_nudge()), (0.0, 0.0), "{what}");
    }
}

#[test]
fn other_characters_keep_the_baseline_and_lose_the_width() {
    let mut text = nudged();
    text.characters = "Other label".to_owned();
    assert_eq!(text.held_baseline_nudge(), BASELINE);
    assert_eq!(text.held_hug_nudge(), 0.0);
}

#[test]
fn what_no_metric_depends_on_keeps_the_nudges() {
    let mut text = nudged();
    text.style.link = Some("https://example.com".to_owned());
    let mut style = text.style.clone();
    style.fills = Some(Vec::new());
    text.runs.push(StyleRun { start: 0, end: 2, style });
    assert!(holds_all(&text));
}

#[test]
fn no_nudge_leaves_no_basis() {
    let mut text = TextData::new("Button".to_owned());
    text.set_nudges(0.0, 0.0);
    assert_eq!(text, TextData::new("Button".to_owned()));
}
