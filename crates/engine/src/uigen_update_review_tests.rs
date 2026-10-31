//! Review fixes of the source updates: the root of a variant is compared too, and the originals follow
//! the version of the copy.

use figcraft_doc::{Document, NodeId};
use serde_json::{Value, json};

use crate::params::id_of;
use crate::uigen_import_support::{Bench, Evolving, EvolvingSnapshot, IMPORT, NEXT_VERSION, PACKAGE, evolving_button, synthetic_button};
use crate::{Reference, reference_of};

const UPDATE: &str = "uigen.updateSource";
const DIFF: &str = "uigen.sourceDiff";
const APPLY: &str = "uigen.applyDifferences";
const PLACE: &str = "uigen.placeImported";
const MAX_PUMPS: usize = 64;

fn repainted_root(version: &str, style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = evolving_button(version, style, size, state, theme);
    if version == NEXT_VERSION {
        snapshot["root"]["style"]["backgroundColor"] = json!("rgb(200, 0, 0)");
    }
    snapshot
}

fn run(bench: &mut Bench, command: &str, params: &Value) -> Value {
    bench.session.execute(command, params).unwrap_or_else(|error| panic!("{command} {params}: {error}"))
}

fn doc(bench: &Bench) -> &Document {
    bench.session.docs[0].doc.as_ref()
}

fn id_in(result: &Value, key: &str) -> NodeId {
    id_of(&result[key]).unwrap_or_else(|| panic!("no {key} in {result}"))
}

fn pump_all(bench: &mut Bench) {
    for _ in 0..MAX_PUMPS {
        bench.session.pump_capture();
    }
}

struct Lab {
    bench: Bench,
    copy: NodeId,
}

fn recaptured_lab(tag: &str) -> Lab {
    let mut bench = Bench::evolving(tag, Evolving { snapshot: repainted_root, failing_at: None });
    let staged = json!({"package": PACKAGE, "component": "Button", "styles": ["solid"], "sizes": ["md"], "states": ["Default", "Hover"], "themes": ["light"], "stage": true});
    bench.import(&staged).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let imported = event.outcome.clone().unwrap_or_else(|error| panic!("the import failed: {}", error.message));
    let placed = run(&mut bench, PLACE, &json!({"set": id_in(&imported, "set")}));
    let copy = id_in(&placed, "set");
    run(&mut bench, "project.setSourceVersion", &json!({"package": PACKAGE, "version": NEXT_VERSION}));
    pump_all(&mut bench);
    run(&mut bench, UPDATE, &json!({"package": PACKAGE}));
    pump_all(&mut bench);
    Lab { bench, copy }
}

fn variants_of(doc: &Document, set: NodeId) -> Vec<NodeId> {
    doc.node(set).unwrap().children.clone()
}

fn fills_of(doc: &Document, id: NodeId) -> String {
    format!("{:?}", doc.node(id).unwrap().fills)
}

fn references(doc: &Document, set: NodeId) -> Vec<Reference> {
    variants_of(doc, set).into_iter().map(|variant| reference_of(doc, variant).unwrap_or_else(|| panic!("no reference on {variant}"))).collect()
}

// covers: E5-5.1
#[test]
fn a_change_of_the_root_of_a_variant_is_listed_and_applied() {
    let mut lab = recaptured_lab("update-root");
    let before = fills_of(doc(&lab.bench), variants_of(doc(&lab.bench), lab.copy)[0]);

    let listed = run(&mut lab.bench, DIFF, &json!({"copy": lab.copy}));

    let root_fills: Vec<&Value> =
        listed["differences"].as_array().unwrap().iter().filter(|raw| raw["path"] == json!("") && raw["field"] == json!("fill")).collect();
    assert_eq!(root_fills.len(), 2, "one root fill per variant in {listed}");
    assert!(root_fills.iter().all(|raw| raw["kind"] == json!("changed") && raw["state"] == json!("clean")), "{listed}");
    assert!(listed["differences"].as_array().unwrap().iter().all(|raw| raw["path"] != json!("") || raw["field"] != json!("position")), "{listed}");
    let id = root_fills[0]["id"].as_str().unwrap().to_owned();

    let applied = run(&mut lab.bench, APPLY, &json!({"copy": lab.copy, "ids": [id]}));

    assert_eq!(applied["applied"], json!([id]), "{applied}");
    let after = fills_of(doc(&lab.bench), variants_of(doc(&lab.bench), lab.copy)[0]);
    assert_ne!(after, before, "the root fill of the variant took the new value");
}

// covers: E5-5.1
#[test]
fn once_the_version_moves_the_originals_of_the_variants_are_those_of_the_new_capture() {
    let mut lab = recaptured_lab("update-originals");
    assert!(references(doc(&lab.bench), lab.copy).iter().all(|reference| reference.version == "1.0.0"));

    let applied = run(&mut lab.bench, APPLY, &json!({"copy": lab.copy}));

    assert_eq!(applied["version"], json!(NEXT_VERSION), "{applied}");
    assert!(references(doc(&lab.bench), lab.copy).iter().all(|reference| reference.version == NEXT_VERSION));
}

const LOCALIZED_ROLES: &str = r#"{"Label": "Libellé", "Icon": "Icône", "Background": "Arrière-plan"}"#;
const LOCALIZED_LABEL: &str = "Libellé";

fn steady_button(_version: &str, style: &str, size: &str, state: &str, theme: &str) -> Value {
    synthetic_button(style, size, state, theme)
}

fn repainted_label(version: &str, style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = synthetic_button(style, size, state, theme);
    if version == NEXT_VERSION {
        snapshot["root"]["children"][0]["style"]["color"] = json!("rgb(200, 0, 0)");
    }
    snapshot
}

/// Imported in French and placed, then recaptured at the next version with `update`.
fn localized_lab(tag: &str, snapshot: EvolvingSnapshot, update: &Value) -> Lab {
    let mut bench = Bench::evolving(tag, Evolving { snapshot, failing_at: None });
    let roles: Value = serde_json::from_str(LOCALIZED_ROLES).unwrap();
    let staged = json!({"package": PACKAGE, "component": "Button", "styles": ["solid"], "sizes": ["md"], "states": ["Default"], "themes": ["light"], "stage": true, "roleNames": roles});
    bench.import(&staged).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let imported = event.outcome.clone().unwrap_or_else(|error| panic!("the import failed: {}", error.message));
    let placed = run(&mut bench, PLACE, &json!({"set": id_in(&imported, "set")}));
    let copy = id_in(&placed, "set");
    run(&mut bench, "project.setSourceVersion", &json!({"package": PACKAGE, "version": NEXT_VERSION}));
    pump_all(&mut bench);
    run(&mut bench, UPDATE, update);
    pump_all(&mut bench);
    Lab { bench, copy }
}

fn layer_names(doc: &Document, set: NodeId) -> Vec<String> {
    doc.subtree(set).into_iter().map(|id| doc.node(id).unwrap().name.clone()).collect()
}

#[test]
fn a_recapture_of_a_localized_import_has_no_difference() {
    let kept_language = json!({"package": PACKAGE});
    let other_language = json!({"package": PACKAGE, "roleNames": {"Label": "Beschriftung"}});
    let english = json!({"package": PACKAGE, "roleNames": {"Label": "Label"}});
    for (tag, update, shown) in
        [("role-kept", &kept_language, LOCALIZED_LABEL), ("role-other", &other_language, "Beschriftung"), ("role-english", &english, "Label")]
    {
        let mut lab = localized_lab(tag, steady_button, update);
        assert!(layer_names(doc(&lab.bench), lab.copy).iter().any(|name| name == LOCALIZED_LABEL), "{tag}");
        let found = run(&mut lab.bench, DIFF, &json!({"copy": lab.copy}));
        assert_eq!(found["differences"], json!([]), "{tag}: {found}");
        let latest = id_in(&found["latest"], "set");
        assert!(layer_names(doc(&lab.bench), latest).iter().any(|name| name == shown), "{tag}");
    }
}

#[test]
fn a_localized_import_names_its_variables_by_the_english_role() {
    let lab = localized_lab("role-variables", steady_button, &json!({"package": PACKAGE}));
    let names: Vec<&str> = doc(&lab.bench).variables.variables.iter().map(|variable| variable.name.as_str()).collect();
    assert!(names.contains(&"couleur/button/label/texte"), "{names:?}");
    assert!(!names.iter().any(|name| name.contains("libell")), "{names:?}");
}

#[test]
fn a_difference_is_matched_in_english_and_shown_in_the_language_of_the_capture() {
    let mut lab = localized_lab("role-shown", repainted_label, &json!({"package": PACKAGE}));
    let found = run(&mut lab.bench, DIFF, &json!({"copy": lab.copy}));
    let listed = found["differences"].as_array().unwrap();
    let change = listed.iter().find(|difference| difference["field"] == "fill").unwrap_or_else(|| panic!("{found}"));
    assert_eq!(change["path"], json!("Label"), "{found}");
    assert_eq!(change["label"], json!(LOCALIZED_LABEL), "{found}");
    let label = doc(&lab.bench).node(variants_of(doc(&lab.bench), lab.copy)[0]).unwrap().children[0];
    let copy_fill_before = fills_of(doc(&lab.bench), label);
    let applied = run(&mut lab.bench, APPLY, &json!({"copy": lab.copy}));
    assert!(applied["applied"].as_array().is_some_and(|ids| !ids.is_empty()), "{applied}");
    assert_ne!(copy_fill_before, fills_of(doc(&lab.bench), label));
}

#[test]
fn a_renamed_layer_of_a_localized_copy_keeps_its_place_in_the_diff() {
    let mut lab = localized_lab("role-renamed", repainted_label, &json!({"package": PACKAGE}));
    let label = doc(&lab.bench).node(variants_of(doc(&lab.bench), lab.copy)[0]).unwrap().children[0];
    run(&mut lab.bench, "node.set", &json!({"ids": [label], "props": {"name": "Texte du bouton"}}));
    let found = run(&mut lab.bench, DIFF, &json!({"copy": lab.copy}));
    let listed = found["differences"].as_array().unwrap();
    assert!(listed.iter().all(|difference| difference["kind"] == "changed" && difference["state"] == "clean"), "{found}");
}
