use figcraft_color::{BlendMode, Color, GradientKind, Paint, PaintKind};
use figcraft_doc::{Effect, EffectKind};
use serde_json::{Map, Value, json};

use super::support::{Run, bounds, frame, import, mentions};

fn sized(style: Value, width: f64, height: f64) -> Run {
    import(json!({ "type": "element", "tag": "div", "box": bounds(0.0, 0.0, width, height), "style": style }))
}

fn borders(sides: [(&str, &str, &str); 4]) -> Value {
    let mut style = Map::new();
    for (side, (width, kind, color)) in ["Top", "Right", "Bottom", "Left"].into_iter().zip(sides) {
        style.insert(format!("border{side}Width"), json!(width));
        style.insert(format!("border{side}Style"), json!(kind));
        style.insert(format!("border{side}Color"), json!(color));
    }
    Value::Object(style)
}

fn fills(run: &Run) -> Vec<[u8; 4]> {
    run.root_node().fills.iter().filter_map(|paint| paint.solid_color()).map(Color::to_rgba8).collect()
}

// covers: T10
#[test]
fn every_computed_colour_form_gives_the_same_fill() {
    let forms = [
        ("rgb(25, 118, 210)", [25, 118, 210, 255]),
        ("rgba(25, 118, 210, 0.5)", [25, 118, 210, 128]),
        ("rgb(25 118 210 / 0.9)", [25, 118, 210, 230]),
        ("hsl(210 79% 46% / 0.9)", [25, 117, 210, 230]),
        ("oklch(0.6 0.2 30 / 0.9)", [222, 62, 45, 230]),
        ("oklab(0.205 0 0)", [23, 23, 23, 255]),
        ("color(srgb 0.0941176 0.0941176 0.105882 / 0.9)", [24, 24, 27, 230]),
        ("color(display-p3 1 1 1 / 0.5)", [255, 255, 255, 128]),
    ];
    for (css, expected) in forms {
        let run = frame(json!({ "backgroundColor": css }));
        let [actual] = fills(&run)[..] else { panic!("{css}: one fill expected, got {:?}", run.root_node().fills) };
        assert!(actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 1), "{css}: {actual:?} vs {expected:?}");
        assert!(run.warnings.is_empty(), "{css}: {:?}", run.warnings);
    }
}

// covers: T9
#[test]
fn a_background_image_no_paint_can_hold_is_reported_not_flattened() {
    let run = frame(json!({ "backgroundColor": "rgb(0, 0, 0)", "backgroundImage": "conic-gradient(rgb(255, 0, 0), rgb(0, 0, 255))" }));
    assert_eq!(fills(&run), [[0, 0, 0, 255]]);
    assert!(mentions(&run.warnings, "Bouton", "backgroundImage"), "{:?}", run.warnings);
    assert!(frame(json!({ "backgroundImage": "none" })).warnings.is_empty());
}

// covers: T3
#[test]
fn sides_of_another_colour_are_left_out_and_named() {
    let run = frame(borders([
        ("1px", "solid", "rgb(255, 0, 0)"),
        ("1px", "solid", "rgb(255, 0, 0)"),
        ("1px", "solid", "rgb(255, 0, 0)"),
        ("2px", "solid", "rgb(0, 0, 255)"),
    ]));
    let node = run.root_node();
    assert_eq!(node.stroke.sides, Some([1.0, 1.0, 1.0, 0.0]));
    assert_eq!(node.stroke.weight, 1.0);
    assert_eq!(node.strokes.iter().filter_map(|paint| paint.solid_color()).map(Color::to_rgba8).collect::<Vec<_>>(), [[255, 0, 0, 255]]);
    assert!(mentions(&run.warnings, "Bouton", "borderLeftColor"), "{:?}", run.warnings);
}

// covers: T3
#[test]
fn transparent_and_hidden_sides_draw_nothing_without_a_warning() {
    let run = frame(borders([("2px", "solid", "rgba(0, 0, 0, 0)"), ("2px", "hidden", "red"), ("2px", "none", "red"), ("0px", "solid", "red")]));
    assert!(run.root_node().strokes.is_empty());
    assert!(run.warnings.is_empty(), "{:?}", run.warnings);
}

// covers: T3
#[test]
fn one_drawn_side_keeps_the_others_at_zero() {
    let run = frame(borders([("3px", "solid", "red"), ("3px", "none", "red"), ("3px", "none", "red"), ("3px", "none", "red")]));
    let node = run.root_node();
    assert_eq!((node.stroke.weight, node.stroke.sides), (3.0, Some([3.0, 0.0, 0.0, 0.0])));
}

// covers: T9
#[test]
fn border_styles_a_stroke_cannot_draw_are_reported_and_drawn_solid() {
    let run = frame(borders([("2px", "groove", "red"), ("2px", "ridge", "red"), ("2px", "double", "red"), ("2px", "wavy", "red")]));
    assert_eq!(run.root_node().strokes.len(), 1);
    for side in ["Top", "Right", "Bottom", "Left"] {
        assert!(mentions(&run.warnings, "Bouton", &format!("border{side}Style")), "{side}: {:?}", run.warnings);
    }
}

// covers: T9
#[test]
fn a_border_needs_a_readable_width_and_colour() {
    let missing = frame(json!({ "borderTopStyle": "solid", "borderTopColor": "red" }));
    assert!(missing.root_node().strokes.is_empty());
    assert!(missing.warnings.is_empty(), "{:?}", missing.warnings);

    let run = frame(borders([("thick", "solid", "red"), ("-1px", "solid", "red"), ("1px", "solid", "nonsense(1)"), ("1px", "solid", "red")]));
    assert_eq!(run.root_node().stroke.sides, Some([0.0, 0.0, 0.0, 1.0]));
    for property in ["borderTopWidth", "borderRightWidth", "borderBottomColor"] {
        assert!(mentions(&run.warnings, "Bouton", property), "{property}: {:?}", run.warnings);
    }
}

// covers: T3
#[test]
fn border_widths_may_be_in_em() {
    let mut style = borders([("0.5em", "solid", "red"); 4]);
    style["fontSize"] = json!("10px");
    assert_eq!(frame(style).root_node().stroke.weight, 5.0);
}

// covers: T4
#[test]
fn percent_radius_of_a_square_is_a_circle() {
    let run = sized(
        json!({ "borderTopLeftRadius": "50%", "borderTopRightRadius": "50%", "borderBottomRightRadius": "50%", "borderBottomLeftRadius": "50%" }),
        40.0,
        40.0,
    );
    assert_eq!(run.root_node().corners.radii, [20.0; 4]);
    assert!(run.warnings.is_empty(), "{:?}", run.warnings);
}

// covers: T4
#[test]
fn elliptical_radius_comes_out_as_its_smaller_axis_and_is_reported() {
    let run = sized(json!({ "borderTopLeftRadius": "50%", "borderTopRightRadius": "10px 20px", "borderBottomRightRadius": "1em" }), 92.0, 36.0);
    assert_eq!(run.root_node().corners.radii, [18.0, 10.0, 16.0, 0.0]);
    assert!(mentions(&run.warnings, "div", "borderTopLeftRadius"), "{:?}", run.warnings);
    assert!(mentions(&run.warnings, "div", "borderTopRightRadius"), "{:?}", run.warnings);
    assert!(!mentions(&run.warnings, "div", "borderBottomRightRadius"), "{:?}", run.warnings);
}

// covers: T4
#[test]
fn radii_too_big_for_a_side_shrink_together_like_css() {
    let pill = sized(
        json!({ "borderTopLeftRadius": "33554400px", "borderTopRightRadius": "33554400px", "borderBottomRightRadius": "33554400px", "borderBottomLeftRadius": "33554400px" }),
        92.0,
        36.0,
    );
    assert!(pill.root_node().corners.radii.iter().all(|radius| (radius - 18.0).abs() < 1e-6), "{:?}", pill.root_node().corners);

    let left = sized(json!({ "borderTopLeftRadius": "30px", "borderBottomLeftRadius": "30px" }), 92.0, 36.0);
    assert_eq!(left.root_node().corners.radii, [18.0, 0.0, 0.0, 18.0]);

    let flat = sized(json!({ "borderTopLeftRadius": "30px" }), 92.0, 0.0);
    assert_eq!(flat.root_node().corners.radii, [0.0; 4]);
}

// covers: T9
#[test]
fn radii_that_cannot_be_read_are_reported_and_square() {
    let run = frame(json!({ "borderTopLeftRadius": "-4px", "borderTopRightRadius": "1px 2px 3px", "borderBottomRightRadius": "4pt" }));
    assert_eq!(run.root_node().corners.radii, [0.0; 4]);
    for property in ["borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius"] {
        assert!(mentions(&run.warnings, "Bouton", property), "{property}: {:?}", run.warnings);
    }
}

// covers: T6
#[test]
fn every_overflow_that_cuts_content_clips() {
    for (value, clips) in
        [("hidden", true), ("auto", true), ("scroll", true), ("clip", true), ("visible hidden", true), ("VISIBLE", false), ("visible visible", false)]
    {
        let run = frame(json!({ "overflow": value }));
        assert_eq!(run.root_node().clip, clips, "{value}");
        assert!(run.warnings.is_empty(), "{value}: {:?}", run.warnings);
    }
    assert!(!frame(json!({})).root_node().clip, "a frame starts without a clip, as a div does");
    for value in ["banana", "hidden hidden hidden"] {
        let run = frame(json!({ "overflow": value }));
        assert!(!run.root_node().clip, "{value}");
        assert!(mentions(&run.warnings, "Bouton", "overflow"), "{value}: {:?}", run.warnings);
    }
}

// covers: T6
#[test]
fn opacity_takes_numbers_and_percentages_within_range() {
    for (value, expected) in [("0.25", 0.25), ("50%", 0.5), ("2", 1.0), ("-1", 0.0)] {
        let run = frame(json!({ "opacity": value }));
        assert!((run.root_node().opacity - expected).abs() < 1e-6, "{value}: {}", run.root_node().opacity);
    }
    let run = frame(json!({ "opacity": "see-through" }));
    assert_eq!(run.root_node().opacity, 1.0);
    assert!(mentions(&run.warnings, "Bouton", "opacity"), "{:?}", run.warnings);
}

// covers: T9
#[test]
fn outline_and_transition_are_ignored_without_a_warning() {
    let run = frame(json!({
        "outlineWidth": "2px", "outlineStyle": "solid", "outlineColor": "rgb(255, 0, 0)", "outlineOffset": "nonsense",
        "transitionProperty": "all", "transitionDuration": "0.3s", "transitionTimingFunction": "ease", "transitionDelay": "0s",
        "cursor": "pointer"
    }));
    assert!(run.root_node().strokes.is_empty());
    assert!(run.warnings.is_empty(), "{:?}", run.warnings);
}

// covers: T1
#[test]
fn a_frame_is_named_after_its_tag_when_the_label_is_empty_and_names_are_capped() {
    let unlabelled = import(json!({ "type": "element", "tag": "nav", "label": "", "box": bounds(0.0, 0.0, 1.0, 1.0) }));
    assert_eq!(unlabelled.root_node().name, "nav");
    let long = import(json!({ "type": "element", "tag": "div", "label": "é".repeat(1000), "box": bounds(0.0, 0.0, 1.0, 1.0) }));
    assert_eq!(long.root_node().name.chars().count(), 128);
}

#[test]
fn a_box_shadow_is_cast_by_the_box() {
    let run = frame(json!({ "boxShadow": "rgb(0, 0, 0) 0px 0px 0px 2px" }));
    let [Effect { kind: EffectKind::DropShadow(shadow), .. }] = &run.root_node().effects[..] else { panic!("{:?}", run.root_node().effects) };
    assert!(shadow.from_outline && !shadow.show_behind);
}

#[test]
fn a_solid_outline_is_a_ring_around_the_box() {
    let run = frame(json!({ "outlineStyle": "solid", "outlineWidth": "2px", "outlineColor": "rgb(255, 0, 0)", "outlineOffset": "0px" }));
    let [Effect { kind: EffectKind::DropShadow(ring), .. }] = &run.root_node().effects[..] else { panic!("{:?}", run.root_node().effects) };
    assert!(ring.from_outline);
    assert_eq!(ring.spread, 2.0);
    assert!(run.warnings.is_empty(), "{:?}", run.warnings);
}

#[test]
fn an_outline_with_a_gap_is_reported() {
    let run = frame(json!({ "outlineStyle": "solid", "outlineWidth": "2px", "outlineColor": "rgb(255, 0, 0)", "outlineOffset": "3px" }));
    assert!(mentions(&run.warnings, "Bouton", "outlineOffset"), "{:?}", run.warnings);
}

#[test]
fn an_outline_of_style_none_paints_nothing() {
    let run = frame(json!({ "outlineStyle": "none", "outlineWidth": "3px", "outlineColor": "rgb(255, 0, 0)" }));
    assert!(run.root_node().effects.is_empty());
}

#[test]
fn a_linear_gradient_is_a_paint_on_the_css_gradient_line() {
    let run = sized(json!({ "backgroundImage": "linear-gradient(90deg, rgb(255, 0, 0) 0%, rgb(0, 0, 255) 100%)" }), 100.0, 50.0);
    let [Paint { kind: PaintKind::Gradient(gradient), .. }] = &run.root_node().fills[..] else { panic!("{:?}", run.root_node().fills) };
    assert_eq!(gradient.kind, GradientKind::Linear);
    assert!((gradient.from.0 - 0.0).abs() < 1e-9 && (gradient.to.0 - 1.0).abs() < 1e-9 && (gradient.from.1 - 0.5).abs() < 1e-9);
    assert_eq!(gradient.stops.len(), 2);
    assert!(run.warnings.is_empty(), "{:?}", run.warnings);
}

#[test]
fn a_corner_gradient_follows_the_magic_corners_of_its_box() {
    let line = |side: &str| {
        let run = sized(json!({ "backgroundImage": format!("linear-gradient({side}, rgb(255, 0, 0), rgb(0, 0, 255))") }), 200.0, 100.0);
        let [Paint { kind: PaintKind::Gradient(gradient), .. }] = &run.root_node().fills[..] else { panic!("{:?}", run.root_node().fills) };
        (gradient.from, gradient.to)
    };
    let close = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9;
    for (side, from, to) in [
        ("to top right", (0.3, 1.3), (0.7, -0.3)),
        ("to right top", (0.3, 1.3), (0.7, -0.3)),
        ("to bottom right", (0.3, -0.3), (0.7, 1.3)),
        ("to bottom left", (0.7, -0.3), (0.3, 1.3)),
        ("to top left", (0.7, 1.3), (0.3, -0.3)),
    ] {
        let found = line(side);
        assert!(close(found.0, from) && close(found.1, to), "{side}: {found:?}");
    }
}

#[test]
fn background_layers_keep_their_order_over_the_colour() {
    let top = "linear-gradient(rgb(255, 0, 0), rgba(0, 0, 0, 0))";
    let under = "radial-gradient(rgb(0, 255, 0), rgb(0, 0, 255) 50%)";
    let run = frame(json!({ "backgroundColor": "rgb(1, 2, 3)", "backgroundImage": format!("{top}, {under}") }));
    let kinds: Vec<_> = run
        .root_node()
        .fills
        .iter()
        .map(|paint| match &paint.kind {
            PaintKind::Gradient(gradient) => Some(gradient.kind),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, [None, Some(GradientKind::Radial), Some(GradientKind::Linear)]);
}

#[test]
fn a_background_url_and_a_tiled_gradient_are_reported() {
    let run = frame(json!({ "backgroundImage": "url(\"a.png\")" }));
    assert!(mentions(&run.warnings, "Bouton", "backgroundImage"), "{:?}", run.warnings);
    let run = frame(json!({ "backgroundImage": "linear-gradient(rgb(0, 0, 0), rgb(1, 1, 1))", "backgroundSize": "8px 8px" }));
    assert!(mentions(&run.warnings, "Bouton", "backgroundSize"), "{:?}", run.warnings);
}

#[test]
fn filter_blur_and_drop_shadow_are_effects_and_the_others_are_reported() {
    let run = frame(json!({ "filter": "blur(4px) drop-shadow(rgb(255, 0, 0) 1px 2px 3px) grayscale(1)" }));
    let [Effect { kind: EffectKind::LayerBlur(blur), .. }, Effect { kind: EffectKind::DropShadow(shadow), .. }] = &run.root_node().effects[..] else {
        panic!("{:?}", run.root_node().effects)
    };
    assert_eq!(blur.radius, 8.0);
    assert_eq!((shadow.offset, shadow.radius), ((1.0, 2.0), 6.0));
    assert!(mentions(&run.warnings, "Bouton", "filter"), "{:?}", run.warnings);
}

#[test]
fn backdrop_blur_is_a_background_blur_and_saturate_is_reported() {
    let run = frame(json!({ "backdropFilter": "blur(8px) saturate(1.5)" }));
    let [Effect { kind: EffectKind::BackgroundBlur(blur), .. }] = &run.root_node().effects[..] else { panic!("{:?}", run.root_node().effects) };
    assert_eq!(blur.radius, 16.0);
    assert!(mentions(&run.warnings, "Bouton", "backdropFilter"), "{:?}", run.warnings);
}

#[test]
fn mix_blend_mode_sets_the_blend_of_the_layer() {
    assert_eq!(frame(json!({ "mixBlendMode": "multiply" })).root_node().blend, BlendMode::Multiply);
    assert!(mentions(&frame(json!({ "mixBlendMode": "plus-darker" })).warnings, "Bouton", "mixBlendMode"));
}

#[test]
fn clip_path_mask_rotation_and_scale_are_reported_and_a_translation_is_not() {
    for (key, value) in [
        ("clipPath", "circle(50%)"),
        ("maskImage", "url(\"m.svg\")"),
        ("transform", "matrix(0, 1, -1, 0, 0, 0)"),
        ("rotate", "45deg"),
        ("scale", "0.97"),
    ] {
        assert!(mentions(&frame(json!({ key: value })).warnings, "Bouton", key), "{key}");
    }
    assert!(frame(json!({ "transform": "matrix(1, 0, 0, 1, 4, 8)", "clipPath": "none", "scale": "none" })).warnings.is_empty());
}

#[test]
fn mixed_border_styles_stay_solid_with_a_note() {
    let black = "rgb(0, 0, 0)";
    let mixed = frame(borders([("2px", "dashed", black), ("2px", "solid", black), ("2px", "dashed", black), ("2px", "solid", black)]));
    assert!(mixed.root_node().stroke.dashes.is_empty());
    assert!(mentions(&mixed.warnings, "Bouton", "borderTopStyle"), "{:?}", mixed.warnings);
}

#[test]
fn a_dashed_border_dashes_and_a_double_border_stays_solid_with_a_note() {
    let dashed = frame(borders([("2px", "dashed", "rgb(0, 0, 0)"); 4]));
    assert_eq!(dashed.root_node().stroke.dashes, [4.0, 4.0]);
    assert!(dashed.warnings.is_empty(), "{:?}", dashed.warnings);
    let double = frame(borders([("3px", "double", "rgb(0, 0, 0)"); 4]));
    assert_eq!(double.root_node().strokes.len(), 1);
    assert!(mentions(&double.warnings, "Bouton", "borderTopStyle"), "{:?}", double.warnings);
}

#[test]
fn a_scale_to_nothing_needs_no_note() {
    assert!(frame(json!({ "transform": "matrix(0, 0, 0, 1, 0, 0)" })).warnings.is_empty());
}
