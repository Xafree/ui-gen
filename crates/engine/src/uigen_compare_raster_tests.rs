//! The raster `uigen.compare` measures is public, so a screen can show exactly what is compared, and
//! the texts it counts are those the import makes.

use figcraft_doc::{Document, NodeId, NodeKind};
use serde_json::{Value, json};

use crate::params::id_of;
use crate::uigen_compare::library_raster;
use crate::uigen_compare::measure::library_texts;
use crate::uigen_import_support::{Bench, Evolving, IMPORT, PACKAGE, evolving_button};

fn imported_set(tag: &str) -> (Bench, Value) {
    let mut bench = Bench::evolving(tag, Evolving { snapshot: evolving_button, failing_at: None });
    let staged = json!({"package": PACKAGE, "component": "Button", "styles": ["solid"], "sizes": ["md"], "states": ["Default"], "themes": ["light"], "stage": true});
    bench.import(&staged).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let imported = event.outcome.clone().unwrap_or_else(|error| panic!("the import failed: {}", error.message));
    (bench, imported)
}

fn doc(bench: &Bench) -> &Document {
    bench.session.docs[0].doc.as_ref()
}

fn first_variant(bench: &Bench, set: &Value) -> NodeId {
    doc(bench).node(id_of(set).unwrap()).unwrap().children[0]
}

fn first_text(doc: &Document, variant: NodeId) -> NodeId {
    doc.subtree(variant).into_iter().find(|id| matches!(doc.node(*id).unwrap().kind, NodeKind::Text(_))).expect("a text layer")
}

// covers: E5-3.1
#[test]
fn the_library_raster_of_a_variant_has_the_size_of_its_node() {
    let (mut bench, imported) = imported_set("compare-raster");
    let placed = bench.session.execute("uigen.placeImported", &json!({"set": imported["set"]})).unwrap();
    let variant = first_variant(&bench, &placed["set"]);

    let raster = library_raster(doc(&bench), variant, false).unwrap();

    let node = doc(&bench).node(variant).unwrap();
    assert_eq!((f64::from(raster.width), f64::from(raster.height)), (node.width.round(), node.height.round()));
    assert!(raster.rgba.chunks(4).any(|pixel| pixel[3] > 0), "something is painted");
}

// covers: E5-3.2
#[test]
fn a_text_under_a_hidden_parent_is_not_counted_among_the_library_texts() {
    let (mut bench, imported) = imported_set("compare-hidden-parent");
    let variant = first_variant(&bench, &imported["set"]);
    let text = first_text(doc(&bench), variant);
    assert_eq!(library_texts(doc(&bench), variant).len(), 1);
    let wrap = bench.session.execute("frame.create", &json!({"parent": variant, "name": "Wrap", "x": 0, "y": 0, "width": 50, "height": 20})).unwrap();
    let wrap = id_of(&wrap["id"]).unwrap();
    bench.session.execute("node.reparent", &json!({"ids": [text], "parent": wrap})).unwrap();
    bench.session.execute("node.set", &json!({"ids": [wrap], "props": {"visible": false}})).unwrap();

    assert!(library_texts(doc(&bench), variant).is_empty());
}
