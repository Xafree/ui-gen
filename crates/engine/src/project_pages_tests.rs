//! In a library project a page is a component: same name, same life cycle. Outside a project
//! every page rule is unchanged.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use figcraft_doc::{NodeId, NodeKind};
use serde_json::{Value, json};

use crate::components::tests::{ex, idv, node};
use crate::project::{ComponentInfo, component_pages, component_roots};
use crate::{EngineError, Session};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-project-pages-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
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
    assert!(s.in_lib_project());
    (dir, s)
}

fn components(s: &mut Session) -> Vec<Value> {
    ex(s, "project.info", json!({}))["components"].as_array().cloned().unwrap_or_default()
}

fn component_names(s: &mut Session) -> Vec<String> {
    components(s).iter().filter_map(|c| c["name"].as_str().map(str::to_string)).collect()
}

fn entry(s: &mut Session, page: NodeId) -> Value {
    components(s).into_iter().find(|c| c["page"] == json!(page)).unwrap_or(Value::Null)
}

fn add(s: &mut Session, p: Value) -> NodeId {
    idv(&ex(s, "page.add", p))
}

fn frame(s: &mut Session, name: &str) -> NodeId {
    idv(&ex(s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 60, "name": name})))
}

fn mark(s: &mut Session, id: NodeId, role: &str) {
    ex(s, "node.set", json!({"ids": [id], "props": {"pluginData": {"uigen.role": role}}}));
}

fn page_names(s: &mut Session) -> Vec<String> {
    ex(s, "page.list", json!({})).as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap().to_string()).collect()
}

fn no_panic(result: &Result<Value, EngineError>, params: &Value) {
    if let Err(EngineError::Internal { cmd, msg }) = result {
        panic!("{cmd} panicked on {params}: {msg}");
    }
}

// covers: L1-C1
#[test]
fn page_add_in_a_project_names_components_by_counting_non_divider_pages() {
    let (_dir, mut s) = lib_session("c1-default");
    assert_eq!(component_names(&mut s), ["Component 1"]);
    let a = add(&mut s, json!({}));
    assert_eq!(node(&s, a).name, "Component 2");
    assert_eq!(component_names(&mut s), ["Component 1", "Component 2"]);

    let divider = ex(&mut s, "page.add", json!({"name": "-- Forms"}));
    assert_eq!(divider["divider"], json!(true), "{divider}");
    let b = add(&mut s, json!({}));
    assert_eq!(node(&s, b).name, "Component 3", "the divider is not counted");
}

// covers: L1-C1
#[test]
fn page_add_in_a_project_skips_names_already_taken() {
    let (_dir, mut s) = lib_session("c1-taken");
    add(&mut s, json!({"name": "Component 3"}));
    let next = add(&mut s, json!({}));
    assert_eq!(node(&s, next).name, "Component 4");
}

// covers: L1-C1
#[test]
fn page_add_in_a_project_uses_a_valid_template() {
    let (_dir, mut s) = lib_session("c1-template");
    let a = add(&mut s, json!({"template": "Card {n}"}));
    assert_eq!(node(&s, a).name, "Card 2");
    assert_eq!(component_names(&mut s), ["Component 1", "Card 2"]);
}

// covers: L1-C1
#[test]
fn page_add_outside_a_project_keeps_page_names_and_ignores_the_template() {
    let mut s = Session::new();
    assert!(!s.in_lib_project());
    let a = add(&mut s, json!({}));
    assert_eq!(node(&s, a).name, "Page 2");
    let b = add(&mut s, json!({"template": "Card {n}"}));
    assert_eq!(node(&s, b).name, "Page 3");
    assert_eq!(page_names(&mut s), ["Page 1", "Page 2", "Page 3"]);
}

// covers: L1-C2
#[test]
fn page_add_with_a_name_keeps_it_and_a_hyphen_makes_a_divider_that_is_no_component() {
    let (_dir, mut s) = lib_session("c2-name");
    let named = add(&mut s, json!({"name": "Button", "template": "Card {n}"}));
    assert_eq!(node(&s, named).name, "Button", "the name wins over the template");

    let divider = ex(&mut s, "page.add", json!({"name": "-- Inputs"}));
    assert_eq!(divider["divider"], json!(true), "{divider}");
    assert_eq!(component_names(&mut s), ["Component 1", "Button"]);
    assert!(page_names(&mut s).contains(&"-- Inputs".to_string()));
}

// covers: L1-C3
#[test]
fn page_rename_in_a_project_shows_the_new_name_under_the_same_id() {
    let (_dir, mut s) = lib_session("c3-rename");
    let page = s.page().unwrap();
    let before = entry(&mut s, page);
    assert_eq!(before["name"], json!("Component 1"));

    ex(&mut s, "page.rename", json!({"id": page, "name": "Button"}));
    let after = entry(&mut s, page);
    assert_eq!(after["name"], json!("Button"), "{after}");
    assert_eq!(after["page"], before["page"]);
    assert_eq!(component_names(&mut s), ["Button"]);
}

// covers: L1-C3
#[test]
fn page_rename_in_a_project_refuses_an_empty_or_blank_name() {
    let (_dir, mut s) = lib_session("c3-blank");
    let page = s.page().unwrap();
    for name in ["", "   ", "\t\n"] {
        let result = s.execute("page.rename", &json!({"id": page, "name": name}));
        no_panic(&result, &json!({"name": name}));
        assert!(result.is_err(), "{name:?} should be refused");
        assert_eq!(node(&s, page).name, "Component 1");
        assert_eq!(component_names(&mut s), ["Component 1"]);
    }
}

// covers: L1-C3
#[test]
fn page_rename_outside_a_project_is_unchanged() {
    let mut s = Session::new();
    let page = s.page().unwrap();
    ex(&mut s, "page.rename", json!({"id": page, "name": "Cover"}));
    assert_eq!(page_names(&mut s), ["Cover"]);
    assert!(s.execute("page.rename", &json!({"id": page, "name": "  "})).is_ok(), "outside a project a blank name is still accepted");
    assert_eq!(ex(&mut s, "project.info", json!({})), json!({"project": null, "components": []}));
}

// covers: L1-C4
#[test]
fn page_delete_in_a_project_removes_the_component_and_undo_redo_restore_and_remove_it() {
    let (_dir, mut s) = lib_session("c4-delete");
    let second = add(&mut s, json!({"name": "Card"}));
    let drawn = frame(&mut s, "Body");
    let third = add(&mut s, json!({"name": "Tag"}));
    let pages_before = s.doc().unwrap().pages.clone();
    assert_eq!(component_names(&mut s), ["Component 1", "Card", "Tag"]);

    ex(&mut s, "page.delete", json!({"id": second}));
    assert_eq!(component_names(&mut s), ["Component 1", "Tag"]);
    assert!(entry(&mut s, second).is_null());

    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(s.doc().unwrap().pages, pages_before, "same ids, same positions");
    assert_eq!(component_names(&mut s), ["Component 1", "Card", "Tag"]);
    let restored = entry(&mut s, second);
    assert_eq!(restored["layers"], json!(1), "{restored}");
    assert_eq!(node(&s, second).children, [drawn]);
    assert_eq!(node(&s, drawn).name, "Body");
    assert_eq!(entry(&mut s, third)["name"], json!("Tag"));

    ex(&mut s, "edit.redo", json!({}));
    assert_eq!(component_names(&mut s), ["Component 1", "Tag"]);
}

// covers: L1-C4
#[test]
fn page_delete_in_a_project_refuses_the_last_component() {
    let (_dir, mut s) = lib_session("c4-last");
    let only = s.page().unwrap();
    let result = s.execute("page.delete", &json!({"id": only}));
    no_panic(&result, &json!({}));
    assert!(result.is_err());
    assert_eq!(component_names(&mut s), ["Component 1"]);

    ex(&mut s, "page.add", json!({"name": "-- Below"}));
    let result = s.execute("page.delete", &json!({"id": only}));
    assert!(result.is_err(), "a divider is not a component");
    assert_eq!(component_names(&mut s), ["Component 1"]);
}

// covers: L1-C5
#[test]
fn layers_are_the_top_level_layers_without_reference_or_note_roles() {
    let (_dir, mut s) = lib_session("c5-roles");
    let page = s.page().unwrap();
    let first = entry(&mut s, page);
    assert_eq!(first["empty"], json!(true), "{first}");
    assert_eq!(first["layers"], json!(0), "{first}");

    let body = frame(&mut s, "Body");
    ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 10, "height": 10, "parent": body}));
    let reference = frame(&mut s, "Spec sheet");
    mark(&mut s, reference, "reference");
    let note = frame(&mut s, "Remark");
    mark(&mut s, note, "note");
    let other = frame(&mut s, "Tagged otherwise");
    ex(&mut s, "node.set", json!({"ids": [other], "props": {"pluginData": {"uigen.role": "component"}}}));

    let counted = entry(&mut s, page);
    assert_eq!(counted["layers"], json!(2), "{counted}");
    assert_eq!(counted["empty"], json!(false), "{counted}");

    let doc = s.doc().unwrap();
    assert_eq!(component_roots(doc, page), [body, other]);
    let info: Vec<ComponentInfo> = component_pages(doc);
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].page, page);
    assert_eq!(info[0].name, "Component 1");
    assert_eq!(info[0].layers, 2);
    assert!(!info[0].empty);
}

// covers: L1-C5
#[test]
fn a_page_with_only_annotation_layers_is_empty() {
    let (_dir, mut s) = lib_session("c5-only-notes");
    let page = s.page().unwrap();
    let reference = frame(&mut s, "Spec sheet");
    mark(&mut s, reference, "reference");
    let note = frame(&mut s, "Remark");
    mark(&mut s, note, "note");

    let listed = entry(&mut s, page);
    assert_eq!(listed["empty"], json!(true), "{listed}");
    assert_eq!(listed["layers"], json!(0), "{listed}");
    assert!(component_roots(s.doc().unwrap(), page).is_empty());
}

// covers: L1-C5
#[test]
fn annotations_measurements_and_comments_never_count_as_layers() {
    let (_dir, mut s) = lib_session("c5-metadata");
    let page = s.page().unwrap();
    let a = frame(&mut s, "A");
    let b = frame(&mut s, "B");
    let before = entry(&mut s, page);
    assert_eq!(before["layers"], json!(2), "{before}");

    ex(&mut s, "annotation.add", json!({"id": a, "label": "Use the brand colour"}));
    ex(&mut s, "devMode.addMeasurement", json!({"from": a, "to": b}));
    ex(&mut s, "comment.add", json!({"x": 5, "y": 5, "text": "Check the radius"}));

    assert_eq!(entry(&mut s, page), before);
}

// covers: L1-C5
#[test]
fn frames_component_sets_and_sections_all_count_as_layers() {
    let (_dir, mut s) = lib_session("c5-kinds");
    let page = s.page().unwrap();
    let plain = frame(&mut s, "Plain");
    assert_eq!(entry(&mut s, page)["layers"], json!(1));

    let variants: Vec<NodeId> = ["Default", "Hover"]
        .iter()
        .enumerate()
        .map(|(k, state)| {
            let f = idv(&ex(
                &mut s,
                "frame.create",
                json!({"x": 300.0 + k as f64 * 150.0, "y": 0, "width": 100, "height": 40, "name": format!("State={state}")}),
            ));
            idv(&ex(&mut s, "component.create", json!({"ids": [f]})))
        })
        .collect();
    let set = idv(&ex(&mut s, "component.combineVariants", json!({"ids": variants})));
    assert!(matches!(node(&s, set).kind, NodeKind::ComponentSet { .. }));
    assert_eq!(entry(&mut s, page)["layers"], json!(2), "the set is one layer, not its variants");

    let to_section = frame(&mut s, "Wrapper");
    ex(&mut s, "object.convertToSection", json!({"ids": [to_section]}));
    assert!(matches!(node(&s, to_section).kind, NodeKind::Section { .. }));
    let counted = entry(&mut s, page);
    assert_eq!(counted["layers"], json!(3), "{counted}");
    assert_eq!(component_roots(s.doc().unwrap(), page), [plain, set, to_section]);
}

// covers: L1-C6
#[test]
fn drawing_a_frame_on_a_blank_page_makes_it_non_empty_without_component_create() {
    let (_dir, mut s) = lib_session("c6-draw");
    let page = add(&mut s, json!({}));
    assert_eq!(entry(&mut s, page)["empty"], json!(true));

    frame(&mut s, "Drawn");
    let listed = entry(&mut s, page);
    assert_eq!(listed["empty"], json!(false), "{listed}");
    assert_eq!(listed["layers"], json!(1), "{listed}");
}

// covers: L1-C6
#[test]
fn page_add_never_creates_a_component_node() {
    let (_dir, mut s) = lib_session("c6-no-node");
    for _ in 0..3 {
        add(&mut s, json!({}));
    }
    let doc = s.doc().unwrap();
    for page in &doc.pages {
        let has_component = doc.subtree(*page).into_iter().any(|id| doc.node(id).is_some_and(|n| matches!(n.kind, NodeKind::Component { .. })));
        assert!(!has_component, "page {page} holds a Component node");
    }
    assert_eq!(component_names(&mut s).len(), 4);
}

// covers: L1-C7
#[test]
fn page_duplicate_in_a_project_lists_a_copy_of_the_component() {
    let (_dir, mut s) = lib_session("c7-duplicate");
    let page = s.page().unwrap();
    frame(&mut s, "Body");
    let copy = idv(&ex(&mut s, "page.duplicate", json!({"id": page})));
    assert_eq!(component_names(&mut s), ["Component 1", "Copy of Component 1"]);
    let listed = entry(&mut s, copy);
    assert_eq!(listed["name"], json!("Copy of Component 1"), "{listed}");
    assert_eq!(listed["layers"], json!(1), "{listed}");
}

// covers: L1-C7
#[test]
fn page_duplicate_in_an_ordinary_tab_stays_ordinary() {
    let (_dir, mut s) = lib_session("c7-ordinary");
    ex(&mut s, "file.new", json!({}));
    assert!(!s.in_lib_project());
    let page = s.page().unwrap();
    let copy = idv(&ex(&mut s, "page.duplicate", json!({"id": page})));
    assert_eq!(node(&s, copy).name, "Copy of Page 1");
    assert_eq!(page_names(&mut s), ["Page 1", "Copy of Page 1"]);
    assert_eq!(ex(&mut s, "project.info", json!({})), json!({"project": null, "components": []}));
    let added = add(&mut s, json!({}));
    assert_eq!(node(&s, added).name, "Page 3");

    ex(&mut s, "doc.switch", json!({"index": 0}));
    assert!(s.in_lib_project());
    assert_eq!(component_names(&mut s), ["Component 1"]);
}

// covers: L1-C8
#[test]
fn page_add_survives_junk_templates_in_a_project() {
    let without_counter = [
        json!({"template": {"id": 1}}),
        json!({"template": ""}),
        json!({"template": "Card"}),
        json!({"template": 7}),
        json!({"template": null}),
        json!({"template": ["{n}"]}),
        json!({"name": [1e308], "template": {"id": 1}}),
    ];
    for junk in without_counter {
        let (_dir, mut s) = lib_session("c8-no-counter");
        let result = s.execute("page.add", &junk);
        no_panic(&result, &junk);
        if let Ok(created) = result {
            assert_eq!(node(&s, idv(&created)).name, "Component 2", "{junk}");
        }
        assert!(s.doc().unwrap().validate().is_empty(), "{junk}");
    }

    let nasty = [
        json!({"template": "{n}".repeat(5000)}),
        json!({"template": "\u{0}{n}\u{202e}"}),
        json!({"template": "{n}", "name": "\u{0}"}),
        json!({"template": "x".repeat(100_000) + "{n}"}),
    ];
    for junk in nasty {
        let (_dir, mut s) = lib_session("c8-nasty");
        let result = s.execute("page.add", &junk);
        no_panic(&result, &junk);
        assert!(s.doc().unwrap().validate().is_empty(), "{junk}");
        no_panic(&s.execute("project.info", &json!({})), &junk);
    }
}
