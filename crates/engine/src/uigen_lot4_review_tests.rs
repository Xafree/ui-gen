//! Review fixes of lot 4: import pages stay out of the library whatever their name, imported sets
//! are no local components, a staged import keeps the selection, the catalog keeps only values an
//! import accepts, and a page that gives way to a copy hands over everything it held.

use figcraft_doc::{Document, Guide, NodeId, NodeKind};
use serde_json::{Value, json};

use crate::assets::library::{local_components, publishable};
use crate::params::id_of;
use crate::project::{component_pages, is_component_page};
use crate::uigen_import::is_import_page;
use crate::uigen_import_support::{Bench, IMPORT, PACKAGE, Scenario, button_inspect, synthetic_source};
use crate::{Result, Session};

const LIST: &str = "uigen.listComponents";
const PLACE: &str = "uigen.placeImported";
const MAX_PUMPS: usize = 64;

fn bench(tag: &str) -> Bench {
    Bench::new(tag, &[synthetic_source(false)], Scenario::new(button_inspect(true, false)))
}

fn import(bench: &mut Bench, stage: bool) -> Value {
    let params = json!({"package": PACKAGE, "component": "Button", "states": ["Default"], "themes": ["light"], "stage": stage});
    bench.import(&params).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    match &event.outcome {
        Ok(result) => result.clone(),
        Err(error) => panic!("the import failed: {}", error.message),
    }
}

fn id_in(result: &Value, key: &str) -> NodeId {
    id_of(&result[key]).unwrap_or_else(|| panic!("no {key} in {result}"))
}

fn doc_of(bench: &Bench) -> &Document {
    bench.session.docs[0].doc.as_ref()
}

fn run(session: &mut Session, command: &str, params: &Value) -> Value {
    session.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

fn guides_of(doc: &Document, page: NodeId) -> Vec<Guide> {
    match &doc.node(page).unwrap().kind {
        NodeKind::Page { guides, .. } => guides.clone(),
        other => panic!("not a page: {other:?}"),
    }
}

fn list_components(bench: &mut Bench) -> Value {
    run(&mut bench.session, LIST, &json!({"package": PACKAGE}));
    for _ in 0..MAX_PUMPS {
        for event in bench.session.pump_capture() {
            if event.command == LIST {
                return event.outcome.unwrap_or_else(|error| panic!("{}", error.message));
            }
        }
    }
    panic!("no {LIST} event");
}

#[test]
fn a_renamed_import_page_is_still_no_component() {
    let mut bench = bench("renamed-page");
    let page = id_in(&import(&mut bench, true), "page");
    run(&mut bench.session, "page.rename", &json!({"id": page, "name": "My buttons"}));

    let doc = doc_of(&bench);

    assert!(is_import_page(doc, page));
    assert!(!is_component_page(doc, page));
    assert!(component_pages(doc).iter().all(|component| component.page != page));
    let info = run(&mut bench.session, "project.info", &json!({}));
    assert!(!info["components"].to_string().contains("My buttons"), "{info}");
}

#[test]
fn imported_sets_are_no_local_component_and_cannot_be_published() {
    let mut bench = bench("local-components");
    let staged = id_in(&import(&mut bench, true), "set");
    let kept = id_in(&import(&mut bench, false), "set");

    let doc = doc_of(&bench);

    assert!(!local_components(doc).contains(&staged), "a set of an import page is no local component");
    assert_eq!(local_components(doc), vec![kept]);
    assert!(!publishable(doc).components.contains(&staged));
    assert_eq!(publishable(doc).components, vec![kept]);
}

#[test]
fn a_staged_import_keeps_the_selection_of_the_current_page() {
    let mut bench = bench("selection");
    let rectangle = id_of(&run(&mut bench.session, "shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10}))["id"]).unwrap();
    bench.session.set_selection(vec![rectangle]);
    let depth = bench.session.undo_depth();

    import(&mut bench, true);

    assert_eq!(bench.session.docs[0].selection, vec![rectangle]);
    assert_eq!(bench.session.undo_depth().0, depth.0 + 1, "the import stays one undo step");
    run(&mut bench.session, "edit.undo", &json!({}));
    assert_eq!(bench.session.docs[0].selection, vec![rectangle]);
}

#[test]
fn an_unstaged_import_still_selects_its_set() {
    let mut bench = bench("selects");

    let set = id_in(&import(&mut bench, false), "set");

    assert_eq!(bench.session.docs[0].selection, vec![set]);
}

#[test]
fn the_catalog_keeps_only_values_an_import_accepts_once_each() {
    let inspect = json!({
        "package": PACKAGE, "version": "1.0.0", "framework": "react", "notes": [],
        "components": [{"name": "Button", "props": [
            {"name": "variant", "type": "enum", "values": ["solid", "solid", "a,b", "c=d", " pad", "pad ", "outline", "\u{7}"]}
        ]}]
    });

    let mut bench = Bench::new("catalog-values", &[synthetic_source(false)], Scenario::new(Ok(inspect)));

    let listed = list_components(&mut bench);

    let kept = bench.session.inspected(PACKAGE).unwrap();
    assert_eq!(kept.components[0].props[0].values, vec!["solid", "outline"]);
    assert_eq!(listed["notes"].as_array().map(Vec::len), Some(1), "{listed}");
    assert!(kept.notes[0].contains("5 values of `variant`"), "{:?}", kept.notes);
}

#[test]
fn a_page_that_gives_way_to_a_copy_hands_its_guides_and_data_to_the_archive() {
    let mut bench = bench("archive-guides");
    let set = id_in(&import(&mut bench, true), "set");
    let target = bench.session.docs[0].page;
    run(&mut bench.session, "shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10}));
    run(&mut bench.session, "guide.add", &json!({"vertical": true, "offset": 42.0, "page": target}));
    bench.session.doc_mut().unwrap().node_mut(target).unwrap().plugin_data.insert("tool.key".into(), "value".into());
    assert_eq!(guides_of(doc_of(&bench), target).len(), 1);

    let placed: Result<Value> = bench.session.execute(PLACE, &json!({"set": set, "page": target}));
    let archive = id_in(&placed.unwrap()["replaced"], "page");

    let doc = doc_of(&bench);
    assert_eq!(guides_of(doc, archive), vec![Guide { vertical: true, offset: 42.0 }]);
    assert!(guides_of(doc, target).is_empty());
    assert_eq!(doc.node(archive).unwrap().plugin_data.get("tool.key").map(String::as_str), Some("value"));
    assert!(doc.node(target).unwrap().plugin_data.is_empty());

    run(&mut bench.session, "edit.undo", &json!({}));
    let doc = doc_of(&bench);
    assert_eq!(guides_of(doc, target).len(), 1);
    assert_eq!(doc.node(target).unwrap().plugin_data.get("tool.key").map(String::as_str), Some("value"));
}
