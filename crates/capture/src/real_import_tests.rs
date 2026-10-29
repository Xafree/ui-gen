//! The component import end to end: the real capture service (Node, Chromium, the npm registry of
//! this machine) behind a real engine session. Every test is `#[ignore]`d because it downloads:
//! run them by hand with `cargo test -p figcraft-capture real_import -- --ignored --test-threads=1`.

use std::thread;
use std::time::{Duration, Instant};

use figcraft_doc::proto::Trigger;
use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_engine::Session;
use figcraft_engine::capture::{CaptureError, CaptureErrorKind, CaptureEvent};
use serde_json::{Value, json};

use crate::service_tests::TempDir;
use crate::{CaptureService, locate};

const PACKAGE: &str = "@mui/material";
const VERSION: &str = "7.3.11";
const ADD_SOURCE: &str = "project.addSource";
const IMPORT: &str = "uigen.importComponent";
const EFFECT: &str = "uigen.effect";
const JOB_LIMIT: Duration = Duration::from_secs(900);

fn project_with_mui(label: &str) -> (TempDir, Session) {
    let dir = TempDir::new(label);
    let config = locate().expect("node and the sidecar script must be found (UIGEN_NODE, UIGEN_SIDECAR_DIR)");
    let mut session = Session::empty();
    session.set_capture_backend(Box::new(CaptureService::start(config, Box::new(|| {}))));
    let opened = session.execute("project.open", &json!({"path": dir.path().display().to_string(), "create": true})).unwrap();
    assert_eq!(opened["opened"], json!(true), "{opened}");

    let started = session.execute(ADD_SOURCE, &json!({"package": PACKAGE, "version": VERSION})).unwrap();
    let event = wait_for(&mut session, ADD_SOURCE, &started);
    if let Err(error) = event.outcome {
        panic!("the install failed: {}", error.message);
    }
    (dir, session)
}

fn wait_for(session: &mut Session, command: &str, started: &Value) -> CaptureEvent {
    let job = started["job"].as_u64().unwrap_or_else(|| panic!("no job id in {started}"));
    let deadline = Instant::now() + JOB_LIMIT;
    loop {
        if let Some(event) = session.pump_capture().into_iter().find(|event| event.command == command) {
            assert_eq!(event.job, job);
            assert_eq!(event.package, PACKAGE);
            return event;
        }
        assert!(Instant::now() < deadline, "no {command} event within {JOB_LIMIT:?}");
        thread::sleep(Duration::from_millis(50));
    }
}

fn import(session: &mut Session, component: &str) -> CaptureEvent {
    let params = json!({
        "package": PACKAGE,
        "component": component,
        "styles": ["contained", "outlined"],
        "sizes": ["small", "medium"],
        "states": ["Default", "Hover", "Pressed", "Disabled"]
    });
    let started = session.execute(IMPORT, &params).unwrap();
    wait_for(session, IMPORT, &started)
}

fn document(session: &Session) -> &Document {
    session.docs[session.active].doc.as_ref()
}

fn document_text(session: &Session) -> String {
    serde_json::to_string(document(session)).unwrap()
}

fn page_children(session: &Session) -> Vec<NodeId> {
    let doc = document(session);
    doc.node(session.docs[session.active].page).unwrap().children.clone()
}

fn subtree(doc: &Document, id: NodeId, out: &mut Vec<NodeId>) {
    out.push(id);
    for child in &doc.node(id).unwrap().children {
        subtree(doc, *child, out);
    }
}

fn assert_readable(error: &CaptureError) {
    assert!(error.message.trim().len() > 10, "the message is not readable: {:?}", error.message);
}

// covers: E3-8.1
#[test]
#[ignore = "downloads @mui/material from npm and drives Chromium; run by hand"]
fn real_import_of_the_mui_button_gives_sixteen_variants_a_light_dark_theme_a_ripple_and_a_hover_transition() {
    let (_dir, mut session) = project_with_mui("real-import");
    let before = document_text(&session);
    assert!(page_children(&session).is_empty(), "the new project's page is not empty");

    let event = import(&mut session, "Button");
    let result = match event.outcome {
        Ok(result) => result,
        Err(error) => panic!("the import failed: {}", error.message),
    };
    assert_eq!(result["variants"], json!(16), "{result}");
    assert!(result["warnings"].is_array(), "{result}");

    let doc = document(&session);
    let set = figcraft_engine::params::id_of(&result["set"]).unwrap_or_else(|| panic!("no set in {result}"));
    let node = doc.node(set).unwrap();
    assert!(matches!(node.kind, NodeKind::ComponentSet { .. }), "{:?}", node.kind);
    assert_eq!(node.children.len(), 16);

    let collection_id = result["collection"].as_str().unwrap_or_else(|| panic!("no collection in {result}"));
    let collection = doc.variables.collections.iter().find(|collection| collection.id == collection_id).expect("the collection exists");
    assert_eq!(collection.name, format!("{PACKAGE} {VERSION}"));
    let modes: Vec<&str> = collection.modes.iter().map(|mode| mode.name.as_str()).collect();
    assert_eq!(modes, ["Light", "Dark"]);

    let mut layers = Vec::new();
    for variant in &node.children {
        subtree(doc, *variant, &mut layers);
    }
    assert!(
        layers.iter().any(|id| doc.node(*id).unwrap().plugin_data.get(EFFECT).map(String::as_str) == Some("ripple")),
        "no layer is flagged {EFFECT} = ripple"
    );
    let hover_durations: Vec<u32> = node
        .children
        .iter()
        .flat_map(|variant| doc.node(*variant).unwrap().interactions.iter())
        .filter(|interaction| interaction.trigger == Trigger::OnHover)
        .map(|interaction| interaction.duration_ms)
        .collect();
    assert!(hover_durations.iter().any(|duration| *duration > 0), "no OnHover interaction has a duration: {hover_durations:?}");

    session.execute("edit.undo", &json!({})).unwrap();
    assert!(page_children(&session).is_empty(), "one undo does not empty the page");
    assert_eq!(document_text(&session), before);
}

// covers: E3-8.2
#[test]
#[ignore = "downloads @mui/material from npm and drives Chromium; run by hand"]
fn real_import_of_an_unknown_component_fails_with_component_not_found_and_leaves_the_document_alone() {
    let (_dir, mut session) = project_with_mui("real-import-nope");
    let before = document_text(&session);
    let depth = session.undo_depth();

    let event = import(&mut session, "Nope");
    let Err(error) = event.outcome else { panic!("the import of an unknown component succeeded") };

    assert_eq!(error.kind, CaptureErrorKind::ComponentNotFound);
    assert_readable(&error);
    assert!(error.message.contains("Nope"), "{}", error.message);
    assert_eq!(document_text(&session), before);
    assert_eq!(session.undo_depth(), depth);
}
