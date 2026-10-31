use figcraft_doc::{LetterSpacing, LineHeight, NodeKind, TextAlignH, TextAutoResize, TextData, TextDecoration};
use serde_json::{Value, json};

use super::support::{Run, bounds, element_with, import, mentions, text_run};

fn data(run: &Run) -> TextData {
    match &run.child(0).kind {
        NodeKind::Text(data) => data.clone(),
        other => panic!("a text layer was expected, got {other:?}"),
    }
}

fn styled(style: Value) -> (TextData, Vec<String>) {
    let run = text_run(style);
    (data(&run), run.warnings)
}

// covers: T7
#[test]
fn weights_and_slants_pick_the_matching_style_name() {
    let cases = [
        (json!({}), "Regular", 400, false),
        (json!({ "fontWeight": "normal", "fontStyle": "normal" }), "Regular", 400, false),
        (json!({ "fontWeight": "bold" }), "Bold", 700, false),
        (json!({ "fontWeight": "100" }), "Thin", 100, false),
        (json!({ "fontWeight": "300" }), "Light", 300, false),
        (json!({ "fontWeight": "500" }), "Medium", 500, false),
        (json!({ "fontWeight": "600", "fontStyle": "italic" }), "Semi Bold Italic", 600, true),
        (json!({ "fontWeight": "800" }), "Extra Bold", 800, false),
        (json!({ "fontWeight": "950" }), "Black", 900, false),
        (json!({ "fontStyle": "oblique 10deg" }), "Italic", 400, true),
    ];
    for (style, name, weight, italic) in cases {
        let (data, warnings) = styled(style.clone());
        assert_eq!(data.style.font_style, name, "{style}");
        assert_eq!((data.style.weight(), data.style.italic()), (weight, italic), "{style}");
        assert!(warnings.is_empty(), "{style}: {warnings:?}");
    }
}

// covers: T9
#[test]
fn font_values_that_cannot_be_read_are_named_and_keep_the_defaults() {
    let (data, warnings) = styled(json!({ "fontWeight": "heavy", "fontStyle": "slanted", "fontSize": "0px", "fontFamily": "\"\"" }));
    let defaults = figcraft_doc::TextStyle::default();
    assert_eq!(
        (data.style.font_style.as_str(), data.style.font_size, data.style.font_family.as_str()),
        ("Regular", defaults.font_size, defaults.font_family.as_str())
    );
    for property in ["fontWeight", "fontStyle", "fontSize", "fontFamily"] {
        assert!(mentions(&warnings, "Bouton", property), "{property}: {warnings:?}");
    }
    let (_, warnings) = styled(json!({ "fontWeight": "0", "fontSize": "big" }));
    assert!(mentions(&warnings, "Bouton", "fontWeight") && mentions(&warnings, "Bouton", "fontSize"), "{warnings:?}");
    assert!(mentions(&styled(json!({ "fontWeight": "1001" })).1, "Bouton", "fontWeight"));
}

// covers: T7
#[test]
fn the_first_family_of_the_stack_is_taken_without_its_quotes() {
    for (stack, family) in [
        ("\"Roboto\", \"Helvetica\", sans-serif", "Roboto"),
        ("'Segoe UI', Arial", "Segoe UI"),
        ("Inter", "Inter"),
        ("\"A, B\", C", "A, B"),
        ("sans-serif", "sans-serif"),
        ("  Fira Code  ,monospace", "Fira Code"),
    ] {
        let (data, warnings) = styled(json!({ "fontFamily": stack }));
        assert_eq!(data.style.font_family, family, "{stack}");
        assert!(warnings.is_empty(), "{stack}: {warnings:?}");
    }
}

// covers: T7
#[test]
fn line_height_is_normal_a_multiple_or_a_length() {
    let cases = [
        ("normal", LineHeight::Auto),
        ("1.5", LineHeight::Percent(150.0)),
        ("20px", LineHeight::Pixels(20.0)),
        ("150%", LineHeight::Pixels(21.0)),
        ("1.5em", LineHeight::Pixels(21.0)),
    ];
    for (value, expected) in cases {
        let (data, warnings) = styled(json!({ "fontSize": "14px", "lineHeight": value }));
        assert_eq!(data.style.line_height, expected, "{value}");
        assert!(warnings.is_empty(), "{value}: {warnings:?}");
    }
    for value in ["tall", "-1px", "-1", "inf"] {
        let (data, warnings) = styled(json!({ "lineHeight": value }));
        assert_eq!(data.style.line_height, LineHeight::Auto, "{value}");
        assert!(mentions(&warnings, "Bouton", "lineHeight"), "{value}: {warnings:?}");
    }
}

// covers: T7
#[test]
fn letter_spacing_is_normal_or_a_length() {
    let (data, _) = styled(json!({ "letterSpacing": "normal" }));
    assert_eq!(data.style.letter_spacing, LetterSpacing::default());
    let (data, _) = styled(json!({ "fontSize": "10px", "letterSpacing": "0.1em" }));
    assert_eq!(data.style.letter_spacing, LetterSpacing::Pixels(1.0));
    let (data, _) = styled(json!({ "letterSpacing": "-0.5px" }));
    assert_eq!(data.style.letter_spacing, LetterSpacing::Pixels(-0.5));
    let (data, warnings) = styled(json!({ "letterSpacing": "wide" }));
    assert_eq!(data.style.letter_spacing, LetterSpacing::default());
    assert!(mentions(&warnings, "Bouton", "letterSpacing"), "{warnings:?}");
}

// covers: T7
#[test]
fn text_transform_is_applied_to_the_content() {
    let run = import(element_with(
        json!({}),
        json!([{ "type": "text", "text": "hello big world", "box": bounds(0.0, 0.0, 10.0, 10.0), "style": { "textTransform": "capitalize" } }]),
    ));
    assert_eq!(data(&run).characters, "Hello Big World");
    for (transform, expected) in [("uppercase", "BOUTON"), ("lowercase", "bouton"), ("none", "Bouton")] {
        let (data, warnings) = styled(json!({ "textTransform": transform }));
        assert_eq!(data.characters, expected, "{transform}");
        assert!(warnings.is_empty(), "{transform}: {warnings:?}");
    }
    let (data, warnings) = styled(json!({ "textTransform": "full-width" }));
    assert_eq!(data.characters, "Bouton");
    assert!(mentions(&warnings, "Bouton", "textTransform"), "{warnings:?}");
}

// covers: T7
#[test]
fn text_align_maps_to_the_four_alignments() {
    for (value, expected) in [
        ("left", TextAlignH::Left),
        ("start", TextAlignH::Left),
        ("center", TextAlignH::Center),
        ("right", TextAlignH::Right),
        ("end", TextAlignH::Right),
        ("justify", TextAlignH::Justified),
    ] {
        assert_eq!(styled(json!({ "textAlign": value })).0.align_h, expected, "{value}");
    }
    let (data, warnings) = styled(json!({ "textAlign": "diagonal" }));
    assert_eq!(data.align_h, TextAlignH::Left);
    assert!(mentions(&warnings, "Bouton", "textAlign"), "{warnings:?}");
}

// covers: T7
#[test]
fn a_box_taller_than_one_line_wraps_at_its_width() {
    let tall = |height: f64, style: Value| {
        let text = json!({ "type": "text", "text": "Bouton", "box": bounds(30.0, 26.0, 50.0, height), "style": style });
        data(&import(element_with(json!({}), json!([text]))))
    };
    assert_eq!(tall(20.0, json!({ "lineHeight": "20px" })).auto_resize, TextAutoResize::WidthAndHeight);
    assert_eq!(tall(40.0, json!({ "lineHeight": "20px" })).auto_resize, TextAutoResize::Height);
    assert_eq!(tall(20.0, json!({ "fontSize": "14px" })).auto_resize, TextAutoResize::WidthAndHeight);
    assert_eq!(tall(50.0, json!({ "fontSize": "14px", "lineHeight": "normal" })).auto_resize, TextAutoResize::Height);
}

// covers: T7
#[test]
fn text_decoration_is_kept_as_a_decoration_of_the_text() {
    for (value, expected) in [
        ("none", TextDecoration::None),
        ("underline", TextDecoration::Underline),
        ("line-through", TextDecoration::Strikethrough),
        ("underline line-through", TextDecoration::Underline),
    ] {
        let (data, warnings) = styled(json!({ "textDecorationLine": value }));
        assert_eq!(data.style.decoration, expected, "{value}");
        assert!(warnings.is_empty(), "{value}: {warnings:?}");
    }
    let (data, warnings) = styled(json!({ "textDecorationLine": "overline" }));
    assert_eq!(data.style.decoration, TextDecoration::None);
    assert!(mentions(&warnings, "Bouton", "textDecorationLine"), "{warnings:?}");
}

// covers: T9
#[test]
fn a_font_the_browser_used_and_this_machine_lacks_is_named_and_kept() {
    let (data, warnings) = styled(json!({ "platformFont": "Zzz Web Font Only" }));
    assert_eq!(data.style.font_family, "Zzz Web Font Only", "the name stays: the layer is right once the font is installed");
    assert!(warnings.iter().any(|warning| warning.contains("Zzz Web Font Only") && warning.contains("not installed")), "{warnings:?}");
}

fn baseline_in_parent(run: &Run) -> (f64, f64) {
    let node = run.child(0);
    let drawn = figcraft_text::layout_cached(&data(run), None, None);
    (node.transform.as_coeffs()[5], drawn.lines[0].baseline)
}

fn drawn_run(box_: Value, line_box: Option<Value>, baseline: Option<f64>) -> Run {
    let mut text = json!({ "type": "text", "text": "Bouton", "box": box_, "style": { "fontSize": "14px" } });
    if let Some(line_box) = line_box {
        text["lineBox"] = line_box;
    }
    if let Some(baseline) = baseline {
        text["baseline"] = json!(baseline);
    }
    import(element_with(json!({}), json!([text])))
}

// covers: T7
#[test]
fn the_baseline_lands_on_the_pixel_the_browser_drew_it_on() {
    let run = drawn_run(bounds(36.0, 29.0, 49.0, 17.0), Some(bounds(36.0, 25.25, 49.0, 24.5)), Some(42.0));
    let (top, baseline) = baseline_in_parent(&run);
    assert!((top - 6.0).abs() < 1e-6, "the half leading is floored: the line starts 3 px above the glyph box, got {top}");
    assert!((top + baseline - 22.0).abs() < 1e-6, "baseline at {}", top + baseline);
}

// covers: T7
#[test]
fn every_line_of_a_wrapped_text_starts_from_the_browser_baseline() {
    let run = drawn_run(bounds(36.0, 29.0, 49.0, 60.0), None, Some(42.0));
    let (top, baseline) = baseline_in_parent(&run);
    assert!((top + baseline - 22.0).abs() < 1e-6, "baseline at {}", top + baseline);
}

// covers: T7
#[test]
fn a_snapshot_without_baseline_or_with_a_senseless_one_keeps_the_natural_line() {
    let line = || Some(bounds(36.0, 25.25, 49.0, 24.5));
    let nudge = |run: &Run| data(run).baseline_nudge;
    assert_eq!(nudge(&drawn_run(bounds(36.0, 29.0, 49.0, 17.0), line(), None)), 0.0);
    assert_eq!(nudge(&drawn_run(bounds(36.0, 29.0, 49.0, 17.0), line(), Some(9000.0))), 0.0);
    let (top, _) = baseline_in_parent(&drawn_run(bounds(36.0, 29.0, 49.0, 17.0), line(), None));
    assert!((top - 5.0).abs() < 1e-6, "an older snapshot stays on its line box, got {top}");
}

// covers: T7
#[test]
fn text_colour_may_be_transparent_or_left_at_the_default() {
    let run = text_run(json!({ "color": "rgba(0, 0, 0, 0)" }));
    assert!(run.child(0).fills.is_empty());
    let run = text_run(json!({}));
    assert_eq!(run.child(0).fills.len(), 1, "text starts black, as a page does");
    assert!(run.warnings.is_empty(), "{:?}", run.warnings);
}

// covers: T9
#[test]
fn a_text_warning_starts_with_the_start_of_the_text() {
    let long = "mot ".repeat(40);
    let text = json!({ "type": "text", "text": long, "box": bounds(0.0, 0.0, 10.0, 10.0), "style": { "color": "nonsense(1)" } });
    let run = import(element_with(json!({}), json!([text])));
    let name: String = long.trim().chars().take(64).collect();
    assert_eq!(run.child(0).name, name);
    assert!(mentions(&run.warnings, &name, "color"), "{:?}", run.warnings);
    assert_eq!(run.child(0).fills.len(), 1, "an unreadable colour leaves the default fill");
}

#[test]
fn an_imported_text_hugs_the_width_the_browser_snapped_it_to() {
    figcraft_text::scan_system_fonts();
    let natural = |text: &TextData| figcraft_text::layout_cached(text, None, None).width;
    let (text, _) = styled(json!({ "fontFamily": "Arial", "fontSize": "13px" }));
    assert!((natural(&text) + text.hug_nudge - 50.0).abs() < 1e-9, "{} {}", natural(&text), text.hug_nudge);
}
