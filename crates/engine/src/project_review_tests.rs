//! Review hardening of the library project: design files match their name whatever the case, a
//! page named with a hyphen is never a component, component names stay unique in a project, and
//! `page.add {template}` is bounded. Outside a project every page rule is unchanged.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use figcraft_doc::NodeId;
use serde_json::{Value, json};

use crate::components::tests::{ex, idv, node};
use crate::project::binding_for_file;
use crate::{EngineError, Session};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-project-review-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
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

fn lib_session(tag: &str) -> (Dir, Session) {
    let dir = Dir::new(tag);
    let mut s = Session::empty();
    let r = s.execute("project.open", &json!({"path": dir.root(), "create": true})).unwrap();
    assert_eq!(r["opened"], json!(true), "{r}");
    (dir, s)
}

fn component_names(s: &mut Session) -> Vec<String> {
    let info = ex(s, "project.info", json!({}));
    info["components"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap().to_string()).collect()
}

fn refusal(s: &mut Session, command: &str, params: Value) -> String {
    match s.execute(command, &params) {
        Err(EngineError::Internal { cmd, msg }) => panic!("{cmd} panicked: {msg}"),
        Err(error) => error.to_string(),
        other => panic!("{command} {params} should be refused, got {other:?}"),
    }
}

fn page_with_a_frame(s: &mut Session, name: &str) -> NodeId {
    let page = idv(&ex(s, "page.add", json!({"name": name})));
    ex(s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 60}));
    page
}

#[test]
fn a_design_file_is_matched_to_the_manifest_whatever_the_case_of_its_name() {
    let dir = Dir::new("case");
    std::fs::write(dir.0.join("uigen.json"), r#"{"version":1,"name":"lib","designFiles":["Lib.figcraft"]}"#).unwrap();
    let listed = dir.0.join("LIB.figcraft");
    std::fs::write(&listed, b"").unwrap();
    let binding = binding_for_file(&listed.to_string_lossy());
    assert!(matches!(binding, Some(Ok(_))), "{binding:?}");
    let other = dir.0.join("other.figcraft");
    assert!(binding_for_file(&other.to_string_lossy()).is_none());
}

// covers: L1-C2
#[test]
fn a_hyphen_page_with_layers_is_no_component_and_deleting_it_is_a_page_deletion() {
    let (_dir, mut s) = lib_session("hyphen-content");
    let page = page_with_a_frame(&mut s, "Temp");
    ex(&mut s, "page.rename", json!({"id": page, "name": "-- Forms"}));
    assert_eq!(component_names(&mut s), ["Component 1"]);
    let next = idv(&ex(&mut s, "page.add", json!({})));
    assert_eq!(node(&s, next).name, "Component 2", "the hyphen page is not counted");
    ex(&mut s, "page.select", json!({"id": next}));
    ex(&mut s, "page.delete", json!({"id": page}));
    assert_eq!(component_names(&mut s), ["Component 1", "Component 2"]);
}

// covers: L1-C2
#[test]
fn the_last_component_cannot_be_deleted_or_renamed_into_a_hyphen_page() {
    let (_dir, mut s) = lib_session("hyphen-last");
    let first = s.page().unwrap();
    page_with_a_frame(&mut s, "-- Forms");
    let deleted = refusal(&mut s, "page.delete", json!({"id": first}));
    assert!(deleted.contains("at least one component"), "{deleted}");
    let renamed = refusal(&mut s, "page.rename", json!({"id": first, "name": "-x"}));
    assert!(renamed.contains("at least one component"), "{renamed}");
    assert_eq!(component_names(&mut s), ["Component 1"]);
}

// covers: L1-C7
#[test]
fn duplicating_a_component_again_takes_a_free_name() {
    let (_dir, mut s) = lib_session("dup");
    let page = s.page().unwrap();
    ex(&mut s, "page.duplicate", json!({"id": page}));
    ex(&mut s, "page.duplicate", json!({"id": page}));
    ex(&mut s, "page.duplicate", json!({"id": page}));
    assert_eq!(component_names(&mut s), ["Component 1", "Copy of Component 1 3", "Copy of Component 1 2", "Copy of Component 1"]);
}

// covers: L1-C3
#[test]
fn renaming_or_adding_a_component_under_a_name_in_use_is_refused_whatever_the_case() {
    let (_dir, mut s) = lib_session("unique");
    let beta = idv(&ex(&mut s, "page.add", json!({"name": "Beta"})));
    let renamed = refusal(&mut s, "page.rename", json!({"id": beta, "name": "component 1"}));
    assert!(renamed.contains("already") && renamed.contains("component 1"), "{renamed}");
    let added = refusal(&mut s, "page.add", json!({"name": "BETA"}));
    assert!(added.contains("already"), "{added}");
    ex(&mut s, "page.rename", json!({"id": beta, "name": "BETA"}));
    ex(&mut s, "page.rename", json!({"id": beta, "name": "Gamma"}));
    assert_eq!(component_names(&mut s), ["Component 1", "Gamma"]);
}

// covers: L1-C3
#[test]
fn outside_a_project_page_names_may_repeat() {
    let mut s = Session::new();
    let page = s.page().unwrap();
    ex(&mut s, "page.duplicate", json!({"id": page}));
    ex(&mut s, "page.duplicate", json!({"id": page}));
    let b = idv(&ex(&mut s, "page.add", json!({"name": "Page 1"})));
    ex(&mut s, "page.rename", json!({"id": b, "name": "Copy of Page 1"}));
    let names: Vec<String> = ex(&mut s, "page.list", json!({})).as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, ["Page 1", "Copy of Page 1", "Copy of Page 1", "Copy of Page 1"]);
}

// covers: L1-C1
#[test]
fn page_add_refuses_a_template_over_the_name_limit() {
    let (_dir, mut s) = lib_session("template");
    let long = format!("{} {{n}}", "x".repeat(200));
    let message = refusal(&mut s, "page.add", json!({"template": long}));
    assert!(message.contains("template") && message.contains("too long"), "{message}");
    let edge = format!("{} {{n}}", "x".repeat(195));
    ex(&mut s, "page.add", json!({"template": edge}));
    assert_eq!(component_names(&mut s).len(), 2);
}

#[test]
fn project_update_refuses_a_blank_name_and_a_dot_directory() {
    let (_dir, mut s) = lib_session("update");
    for params in [json!({"litOutDir": "."}), json!({"litOutDir": ""}), json!({"name": "  "})] {
        refusal(&mut s, "project.update", params);
    }
}
