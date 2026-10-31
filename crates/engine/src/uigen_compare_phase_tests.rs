//! A capture cuts the page on whole pixels: a root the browser scaled sits at a fraction of a pixel in it, and the
//! library is read at that same fraction, on the same rows and columns.

use figcraft_doc::{Document, NodeId};
use serde_json::{Value, json};

use crate::params::id_of;
use crate::reference_of;
use crate::uigen_compare::library_raster;
use crate::uigen_compare::measure::boxes_of;
use crate::uigen_compare::{RefText, TextBox};
use crate::uigen_import_support::{Bench, IMPORT, PACKAGE, Png, Scenario, button_inspect, synthetic_button, synthetic_source};

const X: f64 = 20.25;
const Y: f64 = 20.54;
const WIDTH: f64 = 99.5;
const HEIGHT: f64 = 34.92;

fn pressed_button(style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = synthetic_button(style, size, state, theme);
    snapshot["root"]["box"] = json!({"x": X, "y": Y, "width": WIDTH, "height": HEIGHT});
    snapshot["root"]["style"]["transform"] = json!("matrix(0.97, 0, 0, 0.97, 0, 0)");
    snapshot["root"]["style"]["backgroundColor"] = json!("rgb(0, 0, 0)");
    snapshot["root"]["children"] = json!([]);
    snapshot
}

fn resting_button(style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = pressed_button(style, size, state, theme);
    snapshot["root"]["style"]["transform"] = json!("none");
    snapshot
}

fn imported(tag: &str, snapshot: fn(&str, &str, &str, &str) -> Value) -> (Bench, NodeId) {
    let scenario = Scenario::new(button_inspect(true, false)).png(Png::PerVariant).snapshots(snapshot);
    let mut bench = Bench::new(tag, &[synthetic_source(false)], scenario);
    let request = json!({"package": PACKAGE, "component": "Button", "styles": ["solid"], "sizes": ["md"], "states": ["Default"], "themes": ["light"], "stage": true});
    bench.import(&request).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let outcome = event.outcome.clone().unwrap_or_else(|error| panic!("the import failed: {}", error.message));
    let set = id_of(&outcome["set"]).unwrap();
    let variant = bench.session.docs[0].doc.node(set).unwrap().children[0];
    (bench, variant)
}

fn doc(bench: &Bench) -> &Document {
    bench.session.docs[0].doc.as_ref()
}

fn red(raster: &crate::uigen_compare::Raster, x: u32, y: u32) -> u8 {
    raster.rgba[((y * raster.width + x) * 4) as usize]
}

// covers: E5-3.1
#[test]
fn a_scaled_root_keeps_where_it_falls_in_its_capture() {
    let (bench, variant) = imported("phase-kept", pressed_button);
    let [x, y] = reference_of(doc(&bench), variant).unwrap().themes["light"].phase;
    assert!((x - 0.25).abs() < 1e-9 && (y + 0.46).abs() < 1e-9, "{x} {y}");
}

// covers: E5-3.1
#[test]
fn a_root_on_the_pixel_grid_has_no_phase_to_keep() {
    let (bench, variant) = imported("phase-none", resting_button);
    let reference = reference_of(doc(&bench), variant).unwrap();
    assert_eq!(reference.themes["light"].phase, [0.0, 0.0]);
    assert!(!serde_json::to_string(&reference).unwrap().contains("phase"));
}

// covers: E5-3.1
#[test]
fn a_scaled_root_is_read_on_the_rows_and_columns_of_its_capture() {
    let (bench, variant) = imported("phase-raster", pressed_button);
    let raster = library_raster(doc(&bench), variant, false).unwrap();
    assert_eq!((raster.width, raster.height), (100, 34), "the capture cuts the page at 20..120 and 21..55");
    let inner = red(&raster, 50, 17);
    assert_eq!(inner, 0);
    assert_eq!(red(&raster, 50, 0), inner, "the row the capture starts on is inside the box");
    assert_eq!(red(&raster, 50, 33), inner, "the row the capture ends on is inside the box");
    let edge = red(&raster, 0, 17);
    assert!((60..=70).contains(&edge), "a quarter of the first column is outside the box: {edge}");
}

// covers: E5-3.1
#[test]
fn the_text_boxes_move_with_the_phase() {
    let text = RefText { box_: [10.0, 8.0, 40.0, 16.0], line: Some([10.0, 7.0, 40.0, 18.0]), family: "Inter".to_owned(), size: 14.0, weight: 400 };
    let boxes = boxes_of(&[text], [0.25, -0.5]);
    assert_eq!(boxes, vec![TextBox { box_: [10.25, 7.5, 40.0, 16.0], line: Some([10.25, 6.5, 40.0, 18.0]) }]);
}
