use figcraft_doc::{CrossAlign, LayoutMode, MainAlign};
use serde_json::{Map, Value, json};

use super::support::{bounds, snapshot};
use super::translate_support::{FLEX_COLUMN, FLEX_ROW, NOTE_PREFIX, Translation, offset, parse, translate, translate_with};
use crate::{DomNode, Element, Snapshot, TranslateOptions};

const LAYOUT_TOLERANCE: f64 = 1.0;

fn root_element(snapshot: &Snapshot) -> &Element {
    match &snapshot.root {
        DomNode::Element(element) => element,
        other => panic!("an element root was expected, got {other:?}"),
    }
}

fn captured_boxes(snapshot: &Snapshot) -> Vec<(f64, f64, f64, f64)> {
    let root = root_element(snapshot);
    root.children
        .iter()
        .map(|child| match child {
            DomNode::Element(element) => {
                let captured = element.r#box;
                (captured.x - root.r#box.x, captured.y - root.r#box.y, captured.width, captured.height)
            }
            other => panic!("an element child was expected, got {other:?}"),
        })
        .collect()
}

fn placed_boxes(run: &Translation) -> Vec<(f64, f64, f64, f64)> {
    run.children_of(run.root())
        .into_iter()
        .map(|child| {
            let (x, y) = offset(child);
            (x, y, child.width, child.height)
        })
        .collect()
}

fn assert_on_captured_boxes(run: &Translation, snapshot: &Snapshot, tolerance: f64) {
    let (placed, captured) = (placed_boxes(run), captured_boxes(snapshot));
    assert_eq!(placed.len(), captured.len());
    for (index, (got, want)) in placed.iter().zip(&captured).enumerate() {
        let close = (got.0 - want.0).abs() <= tolerance
            && (got.1 - want.1).abs() <= tolerance
            && (got.2 - want.2).abs() <= tolerance
            && (got.3 - want.3).abs() <= tolerance;
        assert!(close, "child {index}: placed {got:?}, captured {want:?}");
    }
}

fn flex_row(overrides: Value, second_child_x: f64) -> Snapshot {
    let mut style = Map::new();
    for (key, value) in [
        ("display", "flex"),
        ("flexDirection", "row"),
        ("justifyContent", "flex-start"),
        ("alignItems", "flex-start"),
        ("columnGap", "12px"),
        ("rowGap", "0px"),
        ("paddingTop", "4px"),
        ("paddingRight", "8px"),
        ("paddingBottom", "12px"),
        ("paddingLeft", "16px"),
    ] {
        style.insert(key.to_owned(), json!(value));
    }
    if let Some(overrides) = overrides.as_object() {
        style.extend(overrides.clone());
    }
    let first = json!({ "type": "element", "tag": "span", "label": "A", "box": bounds(36.0, 24.0, 60.0, 20.0), "style": { "backgroundColor": "rgb(25, 118, 210)" } });
    let second = json!({ "type": "element", "tag": "span", "label": "B", "box": bounds(20.0 + second_child_x, 24.0, 80.0, 30.0), "style": { "backgroundColor": "rgb(0, 128, 0)" } });
    snapshot(
        json!({ "type": "element", "tag": "div", "label": "Row", "box": bounds(20.0, 20.0, 300.0, 60.0), "style": style, "children": [first, second] }),
    )
}

fn assert_left_absolute_with_note(run: &Translation, snapshot: &Snapshot, property: &str, value: &str) {
    assert!(run.root().layout.is_none(), "{property} {value}: the frame must not get an auto layout");
    assert_on_captured_boxes(run, snapshot, 0.0);
    let expected = format!("{NOTE_PREFIX}{property} {value}");
    let labels = run.note_labels(run.root());
    assert!(labels.iter().any(|label| label.starts_with(&expected)), "{property} {value}: {labels:?}");
    let needle = format!("{property} {value}");
    assert!(run.translated.warnings.iter().any(|warning| warning.contains(&needle)), "{property} {value}: {:?}", run.translated.warnings);
}

// covers: E3-3.1
#[test]
fn a_flex_row_becomes_a_horizontal_auto_layout_frame_with_its_gap_padding_and_alignments() {
    let run = translate(&parse(FLEX_ROW));

    let layout = run.root().layout.as_ref().expect("the row gets an auto layout");
    assert_eq!(layout.mode, LayoutMode::Horizontal);
    assert_eq!(layout.gap, 12.0, "columnGap is the gap along a row");
    assert_eq!(layout.padding, [4.0, 8.0, 12.0, 16.0], "top, right, bottom, left");
    assert_eq!(layout.main_align, MainAlign::Min);
    assert_eq!(layout.cross_align, CrossAlign::Min);
    assert_eq!(run.child_names(run.root()), ["A", "B"]);
}

// covers: E3-3.1
#[test]
fn a_flex_column_becomes_a_vertical_auto_layout_frame_with_its_alignments() {
    let run = translate(&parse(FLEX_COLUMN));

    let layout = run.root().layout.as_ref().expect("the column gets an auto layout");
    assert_eq!(layout.mode, LayoutMode::Vertical);
    assert_eq!(layout.gap, 8.0, "rowGap is the gap along a column");
    assert_eq!(layout.padding, [10.0; 4]);
    assert_eq!(layout.main_align, MainAlign::Center, "justify-content: center");
    assert_eq!(layout.cross_align, CrossAlign::Max, "align-items: flex-end");
}

// covers: E3-3.1
#[test]
fn after_layout_every_child_is_within_a_pixel_of_its_captured_box() {
    for json in [FLEX_ROW, FLEX_COLUMN] {
        let snapshot = parse(json);
        let mut run = translate(&snapshot);
        assert!(run.root().layout.is_some());

        run.lay_out();

        assert_on_captured_boxes(&run, &snapshot, LAYOUT_TOLERANCE);
    }
}

// covers: E3-3.1
#[test]
fn without_the_auto_layout_option_the_frame_is_left_alone() {
    let snapshot = parse(FLEX_ROW);

    let run = translate_with(&snapshot, &TranslateOptions { auto_layout: false, cleanup: true });

    assert!(run.root().layout.is_none());
    assert_on_captured_boxes(&run, &snapshot, 0.0);
}

// covers: E3-3.2
#[test]
fn a_flex_value_that_cannot_be_translated_leaves_the_children_absolute_with_a_note_naming_it() {
    let cases = [
        (json!({ "justifyContent": "space-around" }), "justifyContent", "space-around"),
        (json!({ "flexDirection": "row-reverse" }), "flexDirection", "row-reverse"),
        (json!({ "display": "grid" }), "display", "grid"),
    ];
    for (overrides, property, value) in cases {
        let snapshot = flex_row(overrides, 88.0);

        let run = translate(&snapshot);

        assert_left_absolute_with_note(&run, &snapshot, property, value);
    }
}

// covers: E3-3.2
#[test]
fn positions_the_layout_would_recompute_more_than_a_pixel_away_stay_absolute_with_a_note() {
    let snapshot = flex_row(json!({}), 106.0);

    let run = translate(&snapshot);

    assert!(run.root().layout.is_none(), "the gap says 12 but the second child sits 30 px after the first");
    assert_on_captured_boxes(&run, &snapshot, 0.0);
    assert!(!run.note_labels(run.root()).is_empty(), "the frame carries a note");
    assert!(!run.translated.warnings.is_empty());
}

// covers: E3-3.2
#[test]
fn positions_the_layout_recomputes_within_a_pixel_keep_the_auto_layout() {
    let snapshot = flex_row(json!({}), 89.0);

    let mut run = translate(&snapshot);

    assert!(run.root().layout.is_some());
    assert!(run.note_labels(run.root()).is_empty(), "{:?}", run.note_labels(run.root()));
    run.lay_out();
    assert_on_captured_boxes(&run, &snapshot, LAYOUT_TOLERANCE);
}
