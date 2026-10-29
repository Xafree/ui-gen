//! Review hardening of the manifest checks: the project name and the `.` segment of `litOutDir`.

use serde_json::{Value, json};

use super::*;

fn with(field: &str, value: Value) -> String {
    let mut manifest = json!({"version": 1, "name": "x"});
    manifest[field] = value;
    manifest.to_string()
}

fn names_field(input: &str, field: &str) {
    match Project::parse(input) {
        Err(ProjectError::Invalid { field: named, .. }) => assert_eq!(named, field, "{input}"),
        other => panic!("{input} -> {other:?}"),
    }
}

#[test]
fn refuses_a_name_that_is_blank_has_control_characters_or_is_too_long() {
    for bad in [json!(""), json!("   "), json!("a\nb"), json!("tab\there"), json!("x".repeat(MAX_NAME_CHARS + 1))] {
        names_field(&with("name", bad), "name");
    }
}

#[test]
fn accepts_a_name_up_to_the_limit_with_accents() {
    assert!(Project::parse(&with("name", json!("x".repeat(MAX_NAME_CHARS)))).is_ok());
    assert!(Project::parse(&with("name", json!("Bibliothèque été"))).is_ok());
}

#[test]
fn validate_checks_the_name_of_a_project_built_in_code() {
    let mut project = Project::new("ok");
    project.name = "  ".to_string();
    assert!(matches!(project.validate(), Err(ProjectError::Invalid { field, .. }) if field == "name"));
    project.name = "lib".to_string();
    assert!(project.validate().is_ok());
}

#[test]
fn refuses_a_lit_out_dir_that_is_or_contains_a_dot_segment() {
    for bad in [".", "./lit", "lit/.", "a/./b"] {
        names_field(&with("litOutDir", json!(bad)), "litOutDir");
    }
    assert!(Project::parse(&with("litOutDir", json!("out/lit.v2"))).is_ok());
}

#[test]
fn the_error_text_does_not_repeat_the_file_name() {
    let version = ProjectError::Version { found: 2 }.to_string();
    assert!(version.contains('2') && !version.contains("uigen.json"), "{version}");
    let big = ProjectError::TooLarge { bytes: 5, max: 4 }.to_string();
    assert!(big.contains('5') && !big.contains("uigen.json"), "{big}");
    assert!(!ProjectError::Json("eof".to_string()).to_string().contains("uigen.json"));
}
