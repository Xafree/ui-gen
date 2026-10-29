use figcraft_doc::{EffectKind, Shadow};
use serde_json::json;

use super::support::{frame, mentions};

fn shadows(value: &str) -> (Vec<(bool, Shadow)>, Vec<String>) {
    let run = frame(json!({ "boxShadow": value }));
    let found = run
        .root_node()
        .effects
        .iter()
        .filter_map(|effect| match &effect.kind {
            EffectKind::DropShadow(shadow) => Some((false, shadow.clone())),
            EffectKind::InnerShadow(shadow) => Some((true, shadow.clone())),
            _ => None,
        })
        .collect();
    (found, run.warnings)
}

// covers: T5
#[test]
fn chromium_puts_the_colour_first_and_inset_last() {
    let (found, warnings) = shadows("rgba(0, 0, 0, 0.2) 0px 3px 1px -2px, rgb(255, 0, 0) 1px 2px 3px 4px inset");
    assert!(warnings.is_empty(), "{warnings:?}");
    let [(inset_a, a), (inset_b, b)] = found.as_slice() else { panic!("two shadows expected: {found:?}") };
    assert!(*inset_a, "the later entry is painted first");
    assert_eq!((b.offset, b.radius, b.spread), ((0.0, 3.0), 1.0, -2.0));
    assert_eq!((a.offset, a.radius, a.spread), ((1.0, 2.0), 3.0, 4.0));
    assert!(!inset_b);
}

// covers: T5
#[test]
fn the_first_entry_of_a_list_ends_up_on_top() {
    let (found, _) = shadows("0 1px red, 0 2px green, 0 3px blue");
    let offsets: Vec<f64> = found.iter().map(|(_, shadow)| shadow.offset.1).collect();
    assert_eq!(offsets, [3.0, 2.0, 1.0]);
}

// covers: T5
#[test]
fn blur_spread_and_colour_default_when_left_out() {
    let (found, warnings) = shadows("2px 4px");
    assert!(warnings.is_empty(), "{warnings:?}");
    let [(false, shadow)] = found.as_slice() else { panic!("{found:?}") };
    assert_eq!((shadow.offset, shadow.radius, shadow.spread), ((2.0, 4.0), 0.0, 0.0));
    assert_eq!(shadow.color.to_rgba8(), [0, 0, 0, 255]);
}

// covers: T5
#[test]
fn none_and_transparent_shadows_draw_nothing() {
    for value in ["none", "NONE", "0 0 4px transparent", "0px 0px 0px 0px rgba(0, 0, 0, 0)"] {
        let (found, warnings) = shadows(value);
        assert!(found.is_empty(), "{value}: {found:?}");
        assert!(warnings.is_empty(), "{value}: {warnings:?}");
    }
}

// covers: T5
#[test]
fn em_lengths_follow_the_element_font_size() {
    let run = frame(json!({ "fontSize": "10px", "boxShadow": "0 1em 2em red" }));
    let EffectKind::DropShadow(shadow) = &run.root_node().effects[0].kind else { panic!("drop shadow expected") };
    assert_eq!((shadow.offset.1, shadow.radius), (10.0, 20.0));
}

// covers: T9
#[test]
fn a_malformed_entry_is_named_and_the_others_are_kept() {
    let malformed =
        ["5px", "1px 2px 3px 4px 5px", "1px 2px -3px", "1px 2px red blue", "1px 2px inset inset", "1px red 2px", "1px 2 red", "1px 2px nonsense(1)"];
    for entry in malformed {
        let (found, warnings) = shadows(&format!("{entry}, 0 1px 2px blue"));
        assert_eq!(found.len(), 1, "{entry}: {found:?}");
        assert!(mentions(&warnings, "Bouton", "boxShadow"), "{entry}: {warnings:?}");
        assert!(warnings.iter().any(|warning| warning.contains(entry)), "{entry}: {warnings:?}");
    }
}

// covers: T9
#[test]
fn an_unclosed_parenthesis_swallows_the_rest_of_the_list() {
    let (found, warnings) = shadows("1px 2px rgb(0, 0, 0 1px 2px blue");
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

// covers: T9
#[test]
fn a_long_unreadable_entry_is_cut_in_the_warning() {
    let entry = format!("1px 2px {}", "x".repeat(500));
    let (_, warnings) = shadows(&entry);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].chars().count() < 200, "{}", warnings[0]);
    assert!(warnings[0].ends_with('…'), "{}", warnings[0]);
}
