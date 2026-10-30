//! Review fixes of the token themes: a theme that only moves some of the bindings is applied once and
//! then in sync, and a refused apply leaves `uigen.json` alone.

use std::path::PathBuf;

use figcraft_doc::{Document, NodeId, VariableType};
use serde_json::{Value, json};

use crate::params::id_of;
use crate::uigen_import_support::{Bench, Evolving, IMPORT, PACKAGE, evolving_button};

const THEMES: &str = "uigen.themes";
const APPLY: &str = "uigen.applyTheme";
const BRAND: &str = "Brand";

fn run(bench: &mut Bench, command: &str, params: &Value) -> Value {
    bench.session.execute(command, params).unwrap_or_else(|error| panic!("{command} {params}: {error}"))
}

fn doc(bench: &Bench) -> &Document {
    bench.session.docs[0].doc.as_ref()
}

fn subtree(doc: &Document, id: NodeId) -> Vec<NodeId> {
    let mut found = vec![id];
    for child in &doc.node(id).unwrap().children {
        found.extend(subtree(doc, *child));
    }
    found
}

fn manifest(bench: &Bench) -> String {
    let root = bench.session.project().map(|binding| binding.root.clone()).unwrap_or_default();
    std::fs::read_to_string(PathBuf::from(root).join("uigen.json")).unwrap()
}

fn bound_colour_names(doc: &Document, copy: NodeId) -> Vec<String> {
    let mut names = Vec::new();
    for layer in subtree(doc, copy) {
        for variable in doc.node(layer).unwrap().bound.values() {
            let found = doc.variables.variables.iter().find(|candidate| candidate.id == *variable && candidate.kind == VariableType::Color);
            if let Some(found) = found.filter(|found| !names.contains(&found.name)) {
                names.push(found.name.clone());
            }
        }
    }
    names
}

fn lab_with_a_partial_brand(tag: &str) -> Bench {
    let mut bench = Bench::evolving(tag, Evolving { snapshot: evolving_button, failing_at: None });
    let staged = json!({"package": PACKAGE, "component": "Button", "styles": ["solid"], "sizes": ["md"], "states": ["Default"], "themes": ["light"], "stage": true});
    bench.import(&staged).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let imported = event.outcome.clone().unwrap_or_else(|error| panic!("the import failed: {}", error.message));
    let placed = run(&mut bench, "uigen.placeImported", &json!({"set": imported["set"]}));
    let copy = id_of(&placed["set"]).unwrap();
    let names = bound_colour_names(doc(&bench), copy);
    assert!(names.len() > 1, "the copy binds several colours: {names:?}");
    run(&mut bench, "variable.createCollection", &json!({"name": BRAND, "modes": ["Light", "Dark"]}));
    let values = json!({"Light": "#7B1FA2", "Dark": "#CE93D8"});
    run(&mut bench, "variable.create", &json!({"collection": BRAND, "name": names[0], "type": "color", "values": values}));
    bench
}

// covers: E5-4.5
#[test]
fn a_theme_that_moves_only_some_bindings_is_applied_once_then_in_sync_and_unchanged() {
    let mut bench = lab_with_a_partial_brand("theme-partial");

    let first = run(&mut bench, APPLY, &json!({"theme": BRAND}));

    assert_eq!(first["unchanged"], json!(false), "{first}");
    assert!(!first["unmatched"].as_array().unwrap().is_empty(), "other bindings stay on the imported collection: {first}");
    assert_eq!(run(&mut bench, THEMES, &json!({}))["inSync"], json!(true));
    let again = run(&mut bench, APPLY, &json!({"theme": BRAND}));
    assert_eq!(again["unchanged"], json!(true), "{again}");
    assert_eq!(again["rebound"], json!(0), "{again}");
}

// covers: E5-4.6
#[test]
fn applying_a_theme_while_a_version_is_viewed_is_refused_and_uigen_json_keeps_its_theme() {
    let mut bench = lab_with_a_partial_brand("theme-preview");
    let version = run(&mut bench, "file.saveVersion", &json!({"name": "Before"}));
    run(&mut bench, "version.preview", &json!({"id": version["id"]}));
    let before = manifest(&bench);

    let refused = bench.session.execute(APPLY, &json!({"theme": BRAND}));

    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(manifest(&bench), before, "uigen.json was written though the apply was refused");
}
