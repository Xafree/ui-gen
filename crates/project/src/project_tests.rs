use super::*;
use proptest::prelude::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn base() -> Value {
    json!({"version": 1, "name": "x"})
}

fn with(field: &str, value: Value) -> String {
    let mut v = base();
    v[field] = value;
    v.to_string()
}

fn source(package: &str, version: &str) -> Value {
    json!({"package": package, "version": version})
}

fn refusal(input: &str) -> ProjectError {
    Project::parse(input).unwrap_err()
}

fn assert_names_field(input: &str, field: &str) {
    let err = refusal(input);
    assert!(matches!(err, ProjectError::Invalid { .. }), "{input} -> {err:?}");
    assert!(err.to_string().contains(field), "{input} -> {err}");
}

// covers: L1-P1
#[test]
fn parse_minimal_applies_defaults() {
    let p = Project::parse(r#"{"version":1,"name":"x"}"#).unwrap();
    assert_eq!(p.version, 1);
    assert_eq!(p.name, "x");
    assert_eq!(p.design_files, vec!["lib.figcraft".to_string()]);
    assert_eq!(p.tag_prefix, "ui-");
    assert_eq!(p.lit_out_dir, "lit");
    assert_eq!(p.applied_theme, None);
    assert!(p.sources.is_empty());
    assert!(p.extra.is_empty());
}

// covers: L1-P2
#[test]
fn round_trip_keeps_unknown_keys_and_output_is_deterministic() {
    let a = r#"{"version":1,"name":"ma-lib","designFiles":["lib.figcraft","b.figcraft"],
        "tagPrefix":"my-","litOutDir":"out/lit","appliedTheme":"MUI","futureKey":{"a":[1,2,{"b":null}]},
        "sources":[{"package":"@mui/material","version":"7.3.11","framework":"react","license":"MIT",
        "removed":true,"homepage":"https://example.test"}]}"#;
    let b = r#"{"sources":[{"homepage":"https://example.test","removed":true,"license":"MIT",
        "framework":"react","version":"7.3.11","package":"@mui/material"}],"futureKey":{"a":[1,2,{"b":null}]},
        "appliedTheme":"MUI","litOutDir":"out/lit","tagPrefix":"my-","designFiles":["lib.figcraft","b.figcraft"],
        "name":"ma-lib","version":1}"#;

    let p = Project::parse(a).unwrap();
    assert_eq!(p.extra.get("futureKey"), Some(&json!({"a": [1, 2, {"b": null}]})));
    assert_eq!(p.sources.len(), 1);
    assert_eq!(p.sources[0].package, "@mui/material");
    assert_eq!(p.sources[0].version, "7.3.11");
    assert_eq!(p.sources[0].framework.as_deref(), Some("react"));
    assert_eq!(p.sources[0].license.as_deref(), Some("MIT"));
    assert!(p.sources[0].removed);
    assert_eq!(p.sources[0].extra.get("homepage"), Some(&json!("https://example.test")));

    let out = p.to_json().unwrap();
    assert_eq!(Project::parse(&out).unwrap(), p);
    assert_eq!(p.to_json().unwrap(), out);
    assert_eq!(Project::parse(b).unwrap().to_json().unwrap(), out);
    assert!(out.ends_with('\n') && !out.ends_with("\n\n"));
    assert!(out.contains("\n  \"name\": \"ma-lib\""), "{out}");

    let written: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(written["futureKey"], json!({"a": [1, 2, {"b": null}]}));
    assert_eq!(written["sources"][0]["homepage"], json!("https://example.test"));
}

// covers: L1-P3
#[test]
fn refuses_malformed_json_and_non_object_root() {
    assert!(matches!(refusal("{not json"), ProjectError::Json(_)));
    assert!(matches!(refusal(""), ProjectError::Json(_)));
    for root in ["[1]", "42", "\"x\"", "null"] {
        assert!(Project::parse(root).is_err(), "{root}");
    }
}

// covers: L1-P3
#[test]
fn refuses_other_versions_and_names_a_newer_one() {
    let newer = refusal(r#"{"version":2,"name":"x"}"#);
    assert!(matches!(newer, ProjectError::Version { found: 2 }), "{newer:?}");
    assert!(newer.to_string().contains('2'), "{newer}");
    assert!(Project::parse(r#"{"version":0,"name":"x"}"#).is_err());
}

// covers: L1-P3
#[test]
fn refuses_oversized_input() {
    let big = format!("{}{}", " ".repeat(MAX_JSON_BYTES), r#"{"version":1,"name":"x"}"#);
    match refusal(&big) {
        ProjectError::TooLarge { bytes, max } => {
            assert_eq!(bytes, big.len());
            assert_eq!(max, MAX_JSON_BYTES);
        }
        other => panic!("expected TooLarge, got {other:?}"),
    }
}

// covers: L1-P3
#[test]
fn refuses_invalid_tag_prefix() {
    for bad in ["", "UI-", "1ui-", "ui"] {
        assert_names_field(&with("tagPrefix", json!(bad)), "tagPrefix");
    }
    for good in ["ui-", "my-lib-", "a-"] {
        assert!(Project::parse(&with("tagPrefix", json!(good))).is_ok(), "{good}");
    }
}

// covers: L1-P3
#[test]
fn refuses_invalid_lit_out_dir() {
    for bad in ["", "/x", "C:\\x", "..", "../x", "a/../b", "a\\b"] {
        assert_names_field(&with("litOutDir", json!(bad)), "litOutDir");
    }
    assert!(Project::parse(&with("litOutDir", json!("out/lit"))).is_ok());
}

// covers: L1-P3
#[test]
fn refuses_invalid_design_files() {
    let too_many: Vec<String> = (0..=MAX_DESIGN_FILES).map(|i| format!("f{i}.figcraft")).collect();
    let at_limit: Vec<String> = (0..MAX_DESIGN_FILES).map(|i| format!("f{i}.figcraft")).collect();
    let cases = [
        json!([]),
        json!(too_many),
        json!(["A.figcraft", "a.figcraft"]),
        json!(["a/b.figcraft"]),
        json!(["a\\b.figcraft"]),
        json!(["../lib.figcraft"]),
        json!(["lib.json"]),
        json!(["lib"]),
    ];
    for bad in cases {
        assert_names_field(&with("designFiles", bad), "designFiles");
    }
    assert!(Project::parse(&with("designFiles", json!(at_limit))).is_ok());
}

// covers: L1-P3
#[test]
fn refuses_invalid_sources() {
    assert_names_field(&with("sources", json!([{"version": "1.0.0"}])), "package");
    assert_names_field(&with("sources", json!([{"package": "a"}])), "version");
    assert_names_field(&with("sources", json!([source("a", "1.0.0"), source("a", "1.0.0")])), "sources");

    let too_many: Vec<Value> = (0..=MAX_SOURCES).map(|i| source(&format!("p{i}"), "1.0.0")).collect();
    let at_limit: Vec<Value> = (0..MAX_SOURCES).map(|i| source(&format!("p{i}"), "1.0.0")).collect();
    assert_names_field(&with("sources", json!(too_many)), "sources");
    assert!(Project::parse(&with("sources", json!(at_limit))).is_ok());
}

// covers: L1-P3
#[test]
fn refuses_empty_applied_theme() {
    assert_names_field(&with("appliedTheme", json!("")), "appliedTheme");
    assert!(Project::parse(&with("appliedTheme", json!("MUI"))).is_ok());
}

// covers: L1-P4
proptest! {
    #[test]
    fn never_panics_on_arbitrary_input(
        text in any::<String>(),
        bytes in proptest::collection::vec(any::<u8>(), 0..512),
        template in any::<String>(),
        existing in proptest::collection::vec(any::<String>(), 0..8),
        n in any::<usize>(),
        start in 0usize..200_000,
    ) {
        let _ = Project::parse(&text);
        let _ = Project::parse(&String::from_utf8_lossy(&bytes));
        let _ = component_name(&template, n);
        let _ = next_component_name(&template, existing.iter().map(String::as_str), start);
    }
}

// covers: L1-P5
#[test]
fn component_name_fills_the_counter_or_falls_back_to_the_default_template() {
    assert_eq!(component_name("Component {n}", 3), "Component 3");
    assert_eq!(component_name("Card {n}", 12), "Card 12");
    assert_eq!(DEFAULT_COMPONENT_TEMPLATE, "Component {n}");
    assert_eq!(component_name("Page", 3), component_name(DEFAULT_COMPONENT_TEMPLATE, 3));
}

// covers: L1-P5
#[test]
fn next_component_name_picks_the_first_free_number_from_start() {
    let existing = ["Component 1", "Component 2", "Component 4"];
    assert_eq!(next_component_name("Component {n}", existing, 1), "Component 3");
    assert_eq!(next_component_name("Component {n}", existing, 4), "Component 5");
    assert_eq!(next_component_name("Component {n}", [], 1), "Component 1");
    assert_eq!(next_component_name("Component {n}", [], 7), "Component 7");
    assert_eq!(next_component_name("Card {n}", ["Component 1"], 1), "Card 1");
}

// covers: L1-P5
#[test]
fn next_component_name_terminates_when_every_candidate_is_taken() {
    let taken: Vec<String> = (1..=100_001).map(|n| format!("Component {n}")).collect();
    let name = next_component_name("Component {n}", taken.iter().map(String::as_str), 1);
    assert!(!name.is_empty());
}

// covers: L1-P6
#[test]
fn role_of_reads_the_uigen_role_key() {
    let mut data: BTreeMap<String, String> = BTreeMap::new();
    assert!(Role::of(&data).is_none());

    data.insert("other".to_string(), "reference".to_string());
    assert!(Role::of(&data).is_none());

    data.insert(ROLE_KEY.to_string(), "reference".to_string());
    assert!(matches!(Role::of(&data), Some(Role::Reference)));

    data.insert(ROLE_KEY.to_string(), "note".to_string());
    assert!(matches!(Role::of(&data), Some(Role::Note)));

    data.insert(ROLE_KEY.to_string(), "something-else".to_string());
    assert!(Role::of(&data).is_none());

    assert_eq!(ROLE_KEY, "uigen.role");
}
