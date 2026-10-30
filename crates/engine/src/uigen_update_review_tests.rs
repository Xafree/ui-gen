//! Review fixes of the source updates: the root of a variant is compared too, and the originals follow
//! the version of the copy.

use figcraft_doc::{Document, NodeId};
use serde_json::{Value, json};

use crate::params::id_of;
use crate::uigen_import_support::{Bench, Evolving, IMPORT, NEXT_VERSION, PACKAGE, evolving_button};
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
