//! Library projects: a folder with `uigen.json` and its `.figcraft` files opens as a project, every
//! tab of it is bound to it, and `project.open` / `project.info` / `project.update` read and write
//! the manifest only through the automation roots.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use base64::Engine;
use serde_json::{Value, json};

use crate::file_access::{Roots, confined};
use crate::{EngineError, Result, Session};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-project-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    fn str(&self, rel: &str) -> String {
        self.path(rel).to_string_lossy().into_owned()
    }

    fn root(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn design_bytes(page_name: &str) -> Vec<u8> {
    let mut doc = figcraft_doc::Document::new("lib");
    let page = doc.pages[0];
    doc.node_mut(page).unwrap().name = page_name.to_string();
    figcraft_format::save_bytes(&doc, None).unwrap()
}

fn write_design(path: &Path, page_name: &str) {
    std::fs::write(path, design_bytes(page_name)).unwrap();
}

fn write_manifest(dir: &Path, manifest: &Value) {
    std::fs::write(dir.join("uigen.json"), serde_json::to_string_pretty(manifest).unwrap()).unwrap();
}

fn read_manifest(dir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(dir.join("uigen.json")).unwrap()).unwrap()
}

fn entries(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, at: &Path, out: &mut Vec<String>) {
        let Ok(read) = std::fs::read_dir(at) else { return };
        for entry in read.flatten() {
            let path = entry.path();
            out.push(path.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
            if path.is_dir() {
                walk(base, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn same_folder(shown: &str, expected: &Path) -> bool {
    matches!((std::fs::canonicalize(shown), std::fs::canonicalize(expected)), (Ok(a), Ok(b)) if a == b)
}

fn file_name(path: Option<&str>) -> Option<String> {
    path.and_then(|p| Path::new(p).file_name()).map(|n| n.to_string_lossy().into_owned())
}

fn bad_message(result: Result<Value>) -> String {
    match result {
        Err(EngineError::BadParams { msg, .. }) => msg,
        other => panic!("expected BadParams, got {other:?}"),
    }
}

fn error_text(result: Result<Value>) -> String {
    match result {
        Err(EngineError::Internal { cmd, msg }) => panic!("{cmd} panicked: {msg}"),
        Err(EngineError::UnknownCommand(cmd)) => panic!("{cmd} is not registered"),
        Err(e) => e.to_string(),
        Ok(v) => panic!("expected an error, got {v}"),
    }
}

fn two_files_project(tag: &str) -> Dir {
    let dir = Dir::new(tag);
    write_manifest(&dir.0, &json!({"version": 1, "name": "mylib", "designFiles": ["a.figcraft", "b.figcraft"]}));
    write_design(&dir.path("a.figcraft"), "A");
    write_design(&dir.path("b.figcraft"), "B");
    dir
}

fn one_file_project(tag: &str) -> Dir {
    let dir = Dir::new(tag);
    write_manifest(&dir.0, &json!({"version": 1, "name": "mylib"}));
    write_design(&dir.path("lib.figcraft"), "Button");
    dir
}

// covers: L1-E1
#[test]
fn project_open_opens_every_design_file_in_a_bound_tab_and_activates_the_first() {
    let dir = two_files_project("open");
    let mut s = Session::empty();
    let r = s.execute("project.open", &json!({"path": dir.root()})).unwrap();
    assert_eq!(r["opened"], json!(true), "{r}");
    assert_eq!(r["created"], json!(false), "{r}");
    assert_eq!(r["name"], json!("mylib"), "{r}");
    assert!(same_folder(r["root"].as_str().unwrap(), &dir.0), "{r}");
    assert_eq!(r["files"].as_array().map(Vec::len), Some(2), "{r}");
    assert_eq!(s.docs.len(), 2);
    assert_eq!(r["index"], json!(0));
    assert_eq!(s.active, 0);
    assert_eq!(file_name(s.docs[0].path.as_deref()).as_deref(), Some("a.figcraft"));
    assert_eq!(file_name(s.docs[1].path.as_deref()).as_deref(), Some("b.figcraft"));
    for i in 0..2 {
        s.execute("doc.switch", &json!({"index": i})).unwrap();
        assert!(s.in_lib_project(), "tab {i}");
        let binding = s.project().expect("bound");
        assert!(same_folder(&binding.root, &dir.0));
        assert_eq!(binding.config.name, "mylib");
    }
    let info = s.execute("project.info", &json!({})).unwrap();
    assert_eq!(info["project"]["name"], json!("mylib"), "{info}");
    assert_eq!(info["project"]["designFiles"], json!(["a.figcraft", "b.figcraft"]), "{info}");
    assert_eq!(info["project"]["tagPrefix"], json!("ui-"), "{info}");
    assert_eq!(info["project"]["litOutDir"], json!("lit"), "{info}");
    assert!(info["project"]["appliedTheme"].is_null(), "{info}");
    assert_eq!(info["project"]["sources"], json!([]), "{info}");
    assert_eq!(info["components"], json!([]), "{info}");
}

// covers: L1-E1
#[test]
fn project_open_reuses_a_design_file_that_is_already_open_and_activates_the_first_file() {
    let dir = two_files_project("reuse");
    let mut s = Session::empty();
    s.execute("file.open", &json!({"path": dir.str("b.figcraft")})).unwrap();
    assert_eq!(s.docs.len(), 1);
    let r = s.execute("project.open", &json!({"path": dir.root()})).unwrap();
    assert_eq!(s.docs.len(), 2, "b.figcraft is reused, only a.figcraft is added: {r}");
    assert_eq!(file_name(s.docs[0].path.as_deref()).as_deref(), Some("b.figcraft"));
    assert_eq!(file_name(s.docs[1].path.as_deref()).as_deref(), Some("a.figcraft"));
    assert_eq!(r["index"], json!(1), "{r}");
    assert_eq!(s.active, 1);
    s.execute("doc.switch", &json!({"index": 0})).unwrap();
    assert!(s.in_lib_project(), "the reused tab is bound too");
}

// covers: L1-E2
#[test]
fn project_open_without_a_manifest_reports_missing_and_creates_nothing() {
    let dir = Dir::new("missing");
    std::fs::write(dir.path("notes.txt"), "keep").unwrap();
    let before = entries(&dir.0);
    let mut s = Session::empty();
    let r = s.execute("project.open", &json!({"path": dir.root()})).unwrap();
    assert_eq!(r["opened"], json!(false), "{r}");
    assert_eq!(r["missing"], json!(true), "{r}");
    assert!(same_folder(r["root"].as_str().unwrap(), &dir.0), "{r}");
    assert!(s.docs.is_empty());
    assert_eq!(entries(&dir.0), before);
}

// covers: L1-E3
#[test]
fn project_open_with_create_writes_the_manifest_the_work_folder_and_the_first_component_then_opens_it() {
    let parent = Dir::new("create");
    let dir = parent.path("my-lib");
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Session::empty();
    let r = s.execute("project.open", &json!({"path": dir.to_string_lossy(), "create": true})).unwrap();
    assert_eq!(r["opened"], json!(true), "{r}");
    assert_eq!(r["created"], json!(true), "{r}");
    assert_eq!(r["name"], json!("my-lib"), "{r}");
    let found = entries(&dir);
    for expected in ["uigen.json", ".uigen", "lib.figcraft"] {
        assert!(found.iter().any(|e| e == expected), "{expected} in {found:?}");
    }
    let manifest = read_manifest(&dir);
    assert_eq!(manifest["version"], json!(1));
    assert_eq!(manifest["name"], json!("my-lib"));
    let (doc, _) = figcraft_format::load_file(&dir.join("lib.figcraft")).unwrap();
    let names: Vec<&str> = doc.pages.iter().map(|p| doc.node(*p).unwrap().name.as_str()).collect();
    assert_eq!(names, ["Component 1"]);
    assert_eq!(s.docs.len(), 1);
    assert!(s.in_lib_project());
    assert_eq!(file_name(s.docs[0].path.as_deref()).as_deref(), Some("lib.figcraft"));
}

// covers: L1-E3
#[test]
fn project_open_with_create_names_the_first_component_as_asked() {
    let parent = Dir::new("first-component");
    let dir = parent.path("named");
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Session::empty();
    s.execute("project.open", &json!({"path": dir.to_string_lossy(), "create": true, "firstComponent": "Button"})).unwrap();
    let (doc, _) = figcraft_format::load_file(&dir.join("lib.figcraft")).unwrap();
    let names: Vec<&str> = doc.pages.iter().map(|p| doc.node(*p).unwrap().name.as_str()).collect();
    assert_eq!(names, ["Button"]);
}

// covers: L1-E3
#[test]
fn project_open_with_create_overwrites_nothing_and_keeps_unknown_keys() {
    let dir = Dir::new("idempotent");
    write_manifest(&dir.0, &json!({"version": 1, "name": "custom", "x-keep": {"a": 1}}));
    write_design(&dir.path("lib.figcraft"), "Keep");
    let design_before = std::fs::read(dir.path("lib.figcraft")).unwrap();
    let mut s = Session::empty();
    let r = s.execute("project.open", &json!({"path": dir.root(), "create": true, "firstComponent": "Other"})).unwrap();
    assert_eq!(r["opened"], json!(true), "{r}");
    assert_eq!(r["created"], json!(false), "{r}");
    assert_eq!(std::fs::read(dir.path("lib.figcraft")).unwrap(), design_before);
    let manifest = read_manifest(&dir.0);
    assert_eq!(manifest["name"], json!("custom"));
    assert_eq!(manifest["x-keep"], json!({"a": 1}));
    let doc = &s.docs[s.active].doc;
    assert_eq!(doc.node(doc.pages[0]).unwrap().name, "Keep");
}

// covers: L1-E3
#[test]
fn project_open_with_create_needs_an_existing_folder() {
    let parent = Dir::new("no-folder");
    let missing = parent.path("not-there");
    let mut s = Session::empty();
    let e = s.execute("project.open", &json!({"path": missing.to_string_lossy(), "create": true}));
    assert!(!matches!(e, Ok(_) | Err(EngineError::Internal { .. }) | Err(EngineError::UnknownCommand(_))), "{e:?}");
    assert!(!missing.exists(), "the folder was not made");
    assert!(s.docs.is_empty());
}

// covers: L1-E3
#[test]
fn project_open_with_create_writes_the_manifest_last() {
    let dir = Dir::new("manifest-last");
    std::fs::write(dir.path(".uigen"), "a file where the work folder should go").unwrap();
    let mut s = Session::empty();
    let e = s.execute("project.open", &json!({"path": dir.root(), "create": true}));
    assert!(!matches!(e, Ok(_) | Err(EngineError::Internal { .. }) | Err(EngineError::UnknownCommand(_))), "{e:?}");
    assert!(!dir.path("uigen.json").exists(), "a failed creation leaves no manifest: {:?}", entries(&dir.0));
    assert!(s.docs.is_empty());
}

// covers: L1-E4
#[test]
fn project_open_refuses_an_invalid_manifest_naming_the_cause_and_changes_nothing() {
    let dir = Dir::new("invalid");
    write_manifest(&dir.0, &json!({"version": 1, "name": "x", "tagPrefix": "UI-"}));
    write_design(&dir.path("lib.figcraft"), "A");
    let before = (entries(&dir.0), std::fs::read(dir.path("uigen.json")).unwrap());
    let mut s = Session::empty();
    let msg = bad_message(s.execute("project.open", &json!({"path": dir.root()})));
    assert!(msg.contains("uigen.json") && msg.contains("tagPrefix"), "{msg}");
    assert!(s.docs.is_empty());
    assert_eq!((entries(&dir.0), std::fs::read(dir.path("uigen.json")).unwrap()), before);

    std::fs::write(dir.path("uigen.json"), "{not json").unwrap();
    let msg = bad_message(s.execute("project.open", &json!({"path": dir.root()})));
    assert!(msg.contains("uigen.json"), "{msg}");
    assert!(s.docs.is_empty());
    assert_eq!(std::fs::read(dir.path("uigen.json")).unwrap(), b"{not json");
}

// covers: L1-E4
#[test]
fn project_open_refuses_a_missing_design_file_naming_it_and_opens_no_tab() {
    let dir = Dir::new("absent-file");
    write_manifest(&dir.0, &json!({"version": 1, "name": "x", "designFiles": ["lib.figcraft", "gone.figcraft"]}));
    write_design(&dir.path("lib.figcraft"), "A");
    let before = entries(&dir.0);
    let mut s = Session::empty();
    let msg = bad_message(s.execute("project.open", &json!({"path": dir.root()})));
    assert!(msg.contains("gone.figcraft"), "{msg}");
    assert!(s.docs.is_empty(), "no tab, not even for the file that exists");
    assert_eq!(entries(&dir.0), before);
}

// covers: L1-E5
#[test]
fn file_open_binds_a_figcraft_file_listed_in_the_manifest_of_its_folder() {
    let dir = one_file_project("bind");
    let mut s = Session::empty();
    s.execute("file.open", &json!({"path": dir.str("lib.figcraft")})).unwrap();
    assert!(s.in_lib_project());
    let binding = s.project().expect("bound");
    assert!(same_folder(&binding.root, &dir.0));
    assert_eq!(binding.config.name, "mylib");
    let info = s.execute("project.info", &json!({})).unwrap();
    assert_eq!(info["project"]["name"], json!("mylib"), "{info}");
}

// covers: L1-E5
#[test]
fn file_open_leaves_every_other_file_ordinary() {
    let dir = one_file_project("ordinary");
    write_design(&dir.path("other.figcraft"), "Other");
    std::fs::write(dir.path("lib.svg"), r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#).unwrap();
    std::fs::create_dir_all(dir.path("plain")).unwrap();
    write_design(&dir.path("plain/lib.figcraft"), "Plain");
    std::fs::create_dir_all(dir.path("elsewhere")).unwrap();
    write_manifest(&dir.path("elsewhere"), &json!({"version": 1, "name": "other", "designFiles": ["listed.figcraft"]}));
    write_design(&dir.path("elsewhere/lib.figcraft"), "Unlisted");
    let data = base64::engine::general_purpose::STANDARD.encode(design_bytes("Data"));

    let mut s = Session::empty();
    let opens = [
        json!({"path": dir.str("other.figcraft")}),
        json!({"path": dir.str("lib.svg")}),
        json!({"path": dir.str("plain/lib.figcraft")}),
        json!({"path": dir.str("elsewhere/lib.figcraft")}),
        json!({"data": data, "name": "lib.figcraft"}),
    ];
    for params in &opens {
        s.execute("file.open", params).unwrap();
        assert!(!s.in_lib_project(), "{params}");
        assert!(s.project().is_none(), "{params}");
        let info = s.execute("project.info", &json!({})).unwrap();
        assert!(info["project"].is_null(), "{params}: {info}");
    }
}

// covers: L1-E5
#[test]
fn file_open_falls_back_to_an_ordinary_file_and_says_why_when_the_manifest_is_unreadable() {
    let dir = one_file_project("unreadable");
    std::fs::write(dir.path("uigen.json"), "{not json").unwrap();
    let mut s = Session::empty();
    s.execute("file.open", &json!({"path": dir.str("lib.figcraft")})).unwrap();
    assert_eq!(s.docs.len(), 1);
    assert!(!s.in_lib_project());
    let message = s.message.take().expect("the session says why");
    assert!(message.starts_with("uigen.json: ") && message.ends_with("; opened as an ordinary file"), "{message}");
}

// covers: L1-E6
#[test]
fn project_update_rewrites_the_manifest_keeping_the_rest_and_every_tab_sees_it() {
    let dir = Dir::new("update");
    write_manifest(
        &dir.0,
        &json!({"version": 1, "name": "mylib", "designFiles": ["a.figcraft", "b.figcraft"], "tagPrefix": "ui-", "litOutDir": "lit",
            "appliedTheme": "MUI",
            "sources": [{"package": "@mui/material", "version": "7.3.11", "framework": "react", "license": "MIT", "removed": false, "note": "keep me"}],
            "studio": {"k": [1, 2]}}),
    );
    write_design(&dir.path("a.figcraft"), "A");
    write_design(&dir.path("b.figcraft"), "B");
    let mut s = Session::empty();
    s.execute("project.open", &json!({"path": dir.root()})).unwrap();
    let mut on_disk = read_manifest(&dir.0);
    on_disk["later"] = json!(1);
    write_manifest(&dir.0, &on_disk);

    let r = s.execute("project.update", &json!({"tagPrefix": "my-", "litOutDir": "out/lit", "name": "renamed"})).unwrap();
    assert_eq!(r["project"]["tagPrefix"], json!("my-"), "{r}");
    assert_eq!(r["project"]["litOutDir"], json!("out/lit"), "{r}");
    assert_eq!(r["project"]["name"], json!("renamed"), "{r}");
    let manifest = read_manifest(&dir.0);
    assert_eq!(manifest["tagPrefix"], json!("my-"));
    assert_eq!(manifest["litOutDir"], json!("out/lit"));
    assert_eq!(manifest["name"], json!("renamed"));
    assert_eq!(manifest["appliedTheme"], json!("MUI"));
    assert_eq!(manifest["sources"][0]["note"], json!("keep me"));
    assert_eq!(manifest["studio"], json!({"k": [1, 2]}));
    assert_eq!(manifest["later"], json!(1), "the file is read again before it is changed");
    for i in 0..2 {
        s.execute("doc.switch", &json!({"index": i})).unwrap();
        let config = &s.project().expect("bound").config;
        assert_eq!(config.tag_prefix, "my-", "tab {i}");
        assert_eq!(config.lit_out_dir, "out/lit", "tab {i}");
        assert_eq!(config.name, "renamed", "tab {i}");
    }

    let r = s.execute("project.update", &json!({"appliedTheme": null})).unwrap();
    assert!(r["project"]["appliedTheme"].is_null(), "{r}");
    assert!(read_manifest(&dir.0).get("appliedTheme").is_none_or(Value::is_null));
    let info = s.execute("project.info", &json!({})).unwrap();
    assert!(info["project"]["appliedTheme"].is_null(), "{info}");
}

// covers: L1-E6
#[test]
fn project_update_refuses_an_invalid_value_naming_the_field_and_leaves_the_file_intact() {
    let dir = one_file_project("update-invalid");
    let mut s = Session::empty();
    s.execute("project.open", &json!({"path": dir.root()})).unwrap();
    let before = std::fs::read(dir.path("uigen.json")).unwrap();
    for (params, field) in [(json!({"tagPrefix": "UI-"}), "tagPrefix"), (json!({"litOutDir": "../x"}), "litOutDir")] {
        let text = error_text(s.execute("project.update", &params));
        assert!(text.contains(field), "{params}: {text}");
        assert_eq!(std::fs::read(dir.path("uigen.json")).unwrap(), before, "{params}");
        assert_eq!(s.project().expect("bound").config.tag_prefix, "ui-", "{params}");
    }
}

// covers: L1-E6
#[test]
fn project_update_without_a_project_is_a_bad_request() {
    let mut s = Session::new();
    let msg = bad_message(s.execute("project.update", &json!({"tagPrefix": "x-"})));
    assert!(!msg.is_empty());
}

// covers: L1-E7
#[test]
fn project_open_outside_the_automation_roots_is_rejected() {
    let project = one_file_project("roots-outside");
    let allowed = Dir::new("roots-allowed");
    let roots = Arc::new(Roots::new(&[], &[allowed.root()]).unwrap());
    let mut s = Session::empty();
    let text = error_text(confined(Some(&roots), || s.execute("project.open", &json!({"path": project.root()}))));
    assert!(text.contains("automation path rejected"), "{text}");
    assert!(s.docs.is_empty());
}

// covers: L1-E7
#[test]
fn project_open_with_create_writes_nothing_where_the_roots_only_allow_reading() {
    let dir = Dir::new("roots-read-only");
    let roots = Arc::new(Roots::new(&[dir.root()], &[]).unwrap());
    let mut s = Session::empty();
    let text = error_text(confined(Some(&roots), || s.execute("project.open", &json!({"path": dir.root(), "create": true}))));
    assert!(text.contains("automation path rejected"), "{text}");
    assert!(entries(&dir.0).is_empty(), "{:?}", entries(&dir.0));
}

// covers: L1-E7
#[test]
fn project_update_writes_nothing_where_the_roots_only_allow_reading() {
    let dir = one_file_project("roots-update");
    let mut s = Session::empty();
    s.execute("project.open", &json!({"path": dir.root()})).unwrap();
    let before = std::fs::read(dir.path("uigen.json")).unwrap();
    let roots = Arc::new(Roots::new(&[dir.root()], &[]).unwrap());
    let text = error_text(confined(Some(&roots), || s.execute("project.update", &json!({"tagPrefix": "my-"}))));
    assert!(text.contains("automation path rejected"), "{text}");
    assert_eq!(std::fs::read(dir.path("uigen.json")).unwrap(), before);
}

fn assert_no_panic(result: &Result<Value>, command: &str, params: &Value) {
    if let Err(EngineError::Internal { msg, .. }) = result {
        panic!("{command} panicked on {params}: {msg}");
    }
}

// covers: L1-E8
#[test]
fn absurd_parameters_never_panic_nor_write_outside_the_folder() {
    let parent = Dir::new("absurd");
    let root = parent.path("root");
    let outside = parent.path("outside");
    for sub in ["root/c1", "root/c2", "root/c3", "root/c4", "root/proj", "outside"] {
        std::fs::create_dir_all(parent.path(sub)).unwrap();
    }
    write_manifest(&root.join("proj"), &json!({"version": 1, "name": "proj"}));
    write_design(&root.join("proj/lib.figcraft"), "Button");
    let outside_path = outside.to_string_lossy().into_owned();
    let long = "x".repeat(10_000);

    let open_cases = vec![
        json!({}),
        json!({"path": null}),
        json!({"path": 7}),
        json!({"path": ["a"]}),
        json!({"path": ""}),
        json!({"path": "\u{0}"}),
        json!({"path": "x", "create": true}),
        json!({"path": "..", "create": true}),
        json!({"path": "../..", "create": true}),
        json!({"path": outside_path, "create": true}),
        json!({"path": format!("{}/../outside", root.display()), "create": true}),
        json!({"path": format!("{}/nope", root.display()), "create": true}),
        json!({"path": parent.str("root/c1"), "create": "yes"}),
        json!({"path": parent.str("root/c2"), "create": true, "firstComponent": 5}),
        json!({"path": parent.str("root/c3"), "create": true, "firstComponent": ""}),
        json!({"path": parent.str("root/c4"), "create": true, "firstComponent": "..\\..\\evil/../../b"}),
        json!({"path": parent.str("root/c4"), "create": true, "firstComponent": "\u{0}"}),
        json!({"path": parent.str("root/c4"), "create": true, "firstComponent": long}),
        json!({"path": parent.str("root/proj/lib.figcraft")}),
        json!({"path": parent.str("root/proj/uigen.json"), "create": true}),
    ];
    let roots = Arc::new(Roots::new(&[], &[root.to_string_lossy().into_owned()]).unwrap());
    for params in &open_cases {
        let mut s = Session::new();
        let result = confined(Some(&roots), || s.execute("project.open", params));
        assert_no_panic(&result, "project.open", params);
    }
    assert!(entries(&outside).is_empty(), "{:?}", entries(&outside));
    assert_eq!(entries(&parent.0).into_iter().filter(|e| !e.contains('/')).collect::<Vec<_>>(), ["outside", "root"]);

    for params in [json!({}), json!({"path": null}), json!({"path": 7}), json!({"path": ""}), json!({"path": "\u{0}"}), json!({"path": ".."})] {
        let mut s = Session::new();
        assert_no_panic(&s.execute("project.open", &params), "project.open", &params);
    }

    let proj = root.join("proj");
    let junk_values = [
        json!(null),
        json!([1]),
        json!("x"),
        json!(5),
        json!({}),
        json!({"tagPrefix": 5}),
        json!({"tagPrefix": null}),
        json!({"tagPrefix": "\u{0}"}),
        json!({"litOutDir": ["x"]}),
        json!({"litOutDir": "/etc"}),
        json!({"litOutDir": "..\\..\\x"}),
        json!({"litOutDir": "C:\\x"}),
        json!({"name": ""}),
        json!({"name": {"a": 1}}),
        json!({"name": long}),
        json!({"appliedTheme": 5}),
        json!({"appliedTheme": ""}),
        json!({"path": outside_path, "designFiles": ["../x.figcraft"], "root": outside_path}),
    ];
    let mut s = Session::new();
    s.execute("project.open", &json!({"path": proj.to_string_lossy()})).unwrap();
    for params in &junk_values {
        for command in ["project.update", "project.info"] {
            let result = confined(Some(&roots), || s.execute(command, params));
            assert_no_panic(&result, command, params);
        }
    }
    let mut empty = Session::empty();
    for params in &junk_values {
        for command in ["project.update", "project.info"] {
            assert_no_panic(&empty.execute(command, params), command, params);
        }
    }
    assert!(entries(&outside).is_empty());
    let mut fresh = Session::empty();
    let reopened = fresh.execute("project.open", &json!({"path": proj.to_string_lossy()})).unwrap();
    assert_eq!(reopened["opened"], json!(true), "the manifest is still valid: {reopened}");
}

// covers: L1-E9
#[test]
fn binding_a_file_to_a_project_is_not_an_undo_step_and_the_commands_are_not_undoable() {
    let dir = two_files_project("history");
    let mut s = Session::empty();
    s.execute("project.open", &json!({"path": dir.root()})).unwrap();
    s.execute("project.update", &json!({"tagPrefix": "my-"})).unwrap();
    for (i, ds) in s.docs.iter().enumerate() {
        assert!(ds.history.undo.is_empty() && ds.history.redo.is_empty(), "tab {i}");
        assert!(!ds.is_dirty(), "tab {i}");
    }
    for id in ["project.open", "project.update"] {
        let spec = crate::cmd::all_commands().iter().find(|c| c.id == id).unwrap_or_else(|| panic!("{id} is registered"));
        assert!(!spec.history, "{id}");
    }
}

// covers: L1-E9
#[test]
fn closing_the_tab_frees_the_binding_and_duplicating_does_not_copy_it() {
    let dir = one_file_project("lifecycle");
    let mut s = Session::new();
    s.execute("project.open", &json!({"path": dir.root()})).unwrap();
    assert_eq!(s.docs.len(), 2);
    assert_eq!(s.active, 1);
    let held = Arc::clone(s.project().expect("bound"));
    assert!(Arc::strong_count(&held) >= 2);

    s.execute("file.duplicate", &json!({})).unwrap();
    assert_eq!(s.docs.len(), 3);
    assert!(s.project().is_none(), "the copy is an ordinary file");
    assert!(!s.in_lib_project());
    s.execute("doc.switch", &json!({"index": 1})).unwrap();
    assert!(s.in_lib_project(), "the original keeps it");

    s.execute("file.close", &json!({"index": 1})).unwrap();
    assert_eq!(s.docs.len(), 2);
    assert_eq!(Arc::strong_count(&held), 1, "no tab holds the binding any more");
    assert!(s.project().is_none());
    assert!(!s.in_lib_project());
}
