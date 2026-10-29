//! npm package names and versions: only registry names and exact versions reach `uigen.json`.

use serde_json::{Value, json};

use crate::npm::{validate_package_name, validate_version};
use crate::{Project, ProjectError};

fn assert_invalid(result: Result<(), ProjectError>, input: &str) {
    match result {
        Err(ProjectError::Invalid { reason, .. }) => assert!(!reason.is_empty(), "{input:?} has an empty reason"),
        other => panic!("{input:?} -> {other:?}"),
    }
}

fn manifest_with_source(package: &str, version: &str) -> String {
    let mut manifest = json!({"version": 1, "name": "lib"});
    manifest["sources"] = json!([{"package": package, "version": version}]);
    manifest.to_string()
}

fn refused_source_field(input: &str) -> String {
    match Project::parse(input) {
        Err(ProjectError::Invalid { field, reason }) => {
            assert!(!reason.is_empty(), "{input}");
            field
        }
        other => panic!("{input} -> {other:?}"),
    }
}

// covers: E2-4.1
#[test]
fn accepts_registry_package_names() {
    for name in ["react", "@mui/material", "lit", "a-b.c_d~e", "a1"] {
        assert!(validate_package_name(name).is_ok(), "{name}");
    }
    assert!(validate_package_name(&"a".repeat(214)).is_ok());
}

// covers: E2-4.1
#[test]
fn refuses_what_is_not_a_registry_package_name() {
    let too_long = "a".repeat(215);
    for name in [
        "../x",
        "file:../x",
        "git+https://example.com/x.git",
        "github:user/repo",
        "npm:react@1",
        "x.tgz",
        "-x",
        "Mui",
        "a b",
        "",
        "a\nb",
        ".x",
        "_x",
        "@scope",
        too_long.as_str(),
    ] {
        assert_invalid(validate_package_name(name), name);
    }
}

// covers: E2-4.1
#[test]
fn accepts_exact_versions_and_latest_when_allowed() {
    for version in ["7.3.11", "1.0.0-beta.2+build.5", "0.0.1", "latest"] {
        assert!(validate_version(version, true).is_ok(), "{version}");
    }
    assert!(validate_version("7.3.11", false).is_ok());
    assert!(validate_version("1.0.0-beta.2+build.5", false).is_ok());
    let longest = format!("1.0.0-{}", "a".repeat(58));
    assert_eq!(longest.len(), 64);
    assert!(validate_version(&longest, true).is_ok());
}

// covers: E2-4.1
#[test]
fn refuses_ranges_tags_and_incomplete_versions() {
    let too_long = format!("1.0.0-{}", "a".repeat(59));
    assert_eq!(too_long.len(), 65);
    for version in ["^7.0.0", "~1.2.3", ">=1", "*", "1.x", "next", "1.0", "", "1.0.0 || 2.0.0", too_long.as_str()] {
        assert_invalid(validate_version(version, true), version);
        assert_invalid(validate_version(version, false), version);
    }
}

// covers: E2-4.1
#[test]
fn refuses_latest_when_it_is_not_allowed() {
    assert_invalid(validate_version("latest", false), "latest");
}

// covers: E2-4.2
#[test]
fn parse_refuses_a_source_with_an_invalid_package_name() {
    for package in ["../x", "file:../x", "Mui", "-x"] {
        let field = refused_source_field(&manifest_with_source(package, "1.0.0"));
        assert!(field.starts_with("sources[0]"), "{package} -> {field}");
    }
}

// covers: E2-4.2
#[test]
fn parse_refuses_a_source_whose_version_is_not_exact() {
    for version in ["latest", "^7.0.0", "~1.2.3", ">=1", "*", "next", "1.0"] {
        let field = refused_source_field(&manifest_with_source("react", version));
        assert!(field.starts_with("sources[0]"), "{version} -> {field}");
    }
}

// covers: E2-4.2
#[test]
fn parse_reads_a_valid_lot_1_manifest_back_unchanged() {
    let input = json!({
        "version": 1,
        "name": "ma-lib",
        "sources": [
            {"package": "@mui/material", "version": "7.3.11", "framework": "react", "license": "MIT", "removed": false, "note": "keep me"},
            {"package": "lit", "version": "3.3.2-rc.1+build.5", "removed": true}
        ],
        "futureKey": {"a": [1, {"b": Value::Null}]}
    })
    .to_string();
    let project = Project::parse(&input).unwrap();
    assert_eq!(project.sources.len(), 2);
    assert_eq!(project.sources[0].version, "7.3.11");
    let reread = Project::parse(&project.to_json().unwrap()).unwrap();
    assert_eq!(reread, project);
}
