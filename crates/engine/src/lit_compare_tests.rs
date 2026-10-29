//! The pixel comparison contract of the Lit export: `export.litCompare` renders a node and counts
//! the pixels that differ from a PNG, and `export.lit` gives each component the recipes (variant
//! properties, attributes, pseudo-class) a harness needs to put a Lit element in each of its states.

use std::path::PathBuf;

use crate::components::tests::{ex, idv};
use crate::{EngineError, Session};
use figcraft_doc::NodeId;
use figcraft_render::{Pixmap, encode_png, render_node};
use serde_json::{Value, json};

const CMD: &str = "export.litCompare";

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-lit-compare-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn file(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }

    fn names(&self) -> Vec<String> {
        let mut all: Vec<String> = std::fs::read_dir(&self.0).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        all.sort();
        all
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn swatch(s: &mut Session, width: u32, x: f64) -> NodeId {
    idv(&ex(s, "shape.rectangle", json!({"x": x, "y": 0, "width": width, "height": 10, "props": {"name": "Swatch", "fill": "#ff0000"}})))
}

fn rendered(s: &Session, id: NodeId, scale: f64) -> Pixmap {
    render_node(s.doc().unwrap(), id, scale).expect("the node renders")
}

fn save(dir: &Folder, name: &str, pix: &Pixmap) -> String {
    std::fs::write(dir.file(name), encode_png(pix).unwrap()).unwrap();
    dir.file(name)
}

fn set_pixel(pix: &mut Pixmap, x: u32, y: u32, rgba: [u8; 4]) {
    let at = ((y * pix.width() + x) * 4) as usize;
    pix.data_mut()[at..at + 4].copy_from_slice(&rgba);
}

fn shift_red(pix: &mut Pixmap, x: u32, y: u32, delta: u8) {
    let at = ((y * pix.width() + x) * 4) as usize;
    let red = pix.data()[at];
    pix.data_mut()[at] = if red >= delta { red - delta } else { red + delta };
}

fn compare(s: &mut Session, id: NodeId, image: &str, extra: Value) -> Value {
    let mut params = json!({"id": id, "image": image});
    if let (Some(all), Some(more)) = (params.as_object_mut(), extra.as_object()) {
        all.extend(more.clone());
    }
    ex(s, CMD, params)
}

fn differing(v: &Value) -> u64 {
    v["differing"].as_u64().unwrap_or_else(|| panic!("differing is a count: {v}"))
}

#[test]
fn two_identical_images_have_no_differing_pixel() {
    // covers: E6-7.1
    let mut s = Session::new();
    let id = swatch(&mut s, 20, 0.0);
    let dir = Folder::new("identical");
    let pix = rendered(&s, id, 1.0);
    let image = save(&dir, "same.png", &pix);
    let v = compare(&mut s, id, &image, json!({}));
    assert_eq!(differing(&v), 0, "{v}");
    assert_eq!(v["ratio"].as_f64(), Some(0.0), "{v}");
    assert_eq!(v["sizeMismatch"], false, "{v}");
    assert_eq!((v["width"].as_u64(), v["height"].as_u64()), (Some(u64::from(pix.width())), Some(u64::from(pix.height()))), "{v}");

    let twice = rendered(&s, id, 2.0);
    let image_2x = save(&dir, "same-2x.png", &twice);
    let v = compare(&mut s, id, &image_2x, json!({"scale": 2}));
    assert_eq!((differing(&v), v["sizeMismatch"].as_bool()), (0, Some(false)), "the scale renders the node bigger: {v}");
    let v = compare(&mut s, id, &image_2x, json!({}));
    assert_eq!(v["sizeMismatch"], true, "at scale 1 the 2x image does not match: {v}");
}

#[test]
fn one_different_pixel_is_counted_out_of_width_times_height() {
    // covers: E6-7.2
    let mut s = Session::new();
    let id = swatch(&mut s, 20, 0.0);
    let dir = Folder::new("one-pixel");
    let mut pix = rendered(&s, id, 1.0);
    set_pixel(&mut pix, 3, 2, [0, 0, 255, 255]);
    let image = save(&dir, "one.png", &pix);
    let v = compare(&mut s, id, &image, json!({}));
    let total = u64::from(pix.width() * pix.height());
    assert_eq!((differing(&v), v["total"].as_u64()), (1, Some(total)), "{v}");
    assert!((v["ratio"].as_f64().unwrap() - 1.0 / total as f64).abs() < 1e-9, "{v}");
    assert_eq!(v["sizeMismatch"], false, "{v}");
}

#[test]
fn a_gap_under_the_tolerance_is_not_counted_and_one_above_it_is() {
    // covers: E6-7.3
    let mut s = Session::new();
    let id = swatch(&mut s, 20, 0.0);
    let dir = Folder::new("tolerance");
    let exact = save(&dir, "exact.png", &rendered(&s, id, 1.0));
    let mut pix = rendered(&s, id, 1.0);
    shift_red(&mut pix, 10, 5, 5);
    let near = save(&dir, "near.png", &pix);

    assert_eq!(differing(&compare(&mut s, id, &exact, json!({"tolerance": 0}))), 0, "identical stays identical at tolerance 0");
    assert_eq!(differing(&compare(&mut s, id, &near, json!({}))), 0, "a gap of 5 is under the default tolerance of 8");
    assert_eq!(differing(&compare(&mut s, id, &near, json!({"tolerance": 8}))), 0);
    assert_eq!(differing(&compare(&mut s, id, &near, json!({"tolerance": 2}))), 1, "a gap of 5 is above 2");
    assert_eq!(differing(&compare(&mut s, id, &near, json!({"tolerance": 0}))), 1, "any gap is above 0");
}

#[test]
fn an_image_of_another_size_is_a_size_mismatch() {
    // covers: E6-7.4
    let mut s = Session::new();
    let id = swatch(&mut s, 20, 0.0);
    let wider = swatch(&mut s, 30, 100.0);
    let dir = Folder::new("size");
    let image = save(&dir, "wider.png", &rendered(&s, wider, 1.0));
    let v = compare(&mut s, id, &image, json!({}));
    assert_eq!(v["sizeMismatch"], true, "{v}");
}

#[test]
fn a_missing_file_a_non_png_and_a_tolerance_out_of_range_are_refused_without_writing() {
    // covers: E6-7.5
    let mut s = Session::new();
    let id = swatch(&mut s, 20, 0.0);
    let dir = Folder::new("refusals");
    let good = save(&dir, "good.png", &rendered(&s, id, 1.0));
    std::fs::write(dir.file("notes.txt"), b"not an image").unwrap();
    let before = dir.names();
    let cases = [
        json!({"id": id, "image": dir.file("missing.png")}),
        json!({"id": id, "image": dir.file("notes.txt")}),
        json!({"id": id, "image": good, "tolerance": 256}),
        json!({"id": id, "image": good, "tolerance": -1}),
        json!({"id": id, "image": good, "tolerance": 1000}),
    ];
    for params in cases {
        match s.execute(CMD, &params) {
            Err(EngineError::BadParams { cmd, msg }) => assert!(cmd == CMD && !msg.is_empty(), "{params}"),
            other => panic!("{params}: expected a bad-parameters refusal, got {other:?}"),
        }
    }
    assert_eq!(dir.names(), before, "nothing was written");
}

fn state_set(s: &mut Session, name: &str, rows: &[&str]) -> NodeId {
    let mut variants = Vec::new();
    for (k, row) in rows.iter().enumerate() {
        let f = idv(&ex(s, "frame.create", json!({"x": k as f64 * 150.0, "y": 100, "width": 100, "height": 40, "name": row})));
        ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": {"name": "Bg", "fill": "#ff0000"}}));
        variants.push(idv(&ex(s, "component.create", json!({"ids": [f]}))));
    }
    let set = idv(&ex(s, "component.combineVariants", json!({"ids": variants})));
    ex(s, "node.set", json!({"id": set, "props": {"name": name}}));
    set
}

fn recipe<'a>(states: &'a [Value], props: &Value) -> &'a Value {
    states.iter().find(|r| &r["props"] == props).unwrap_or_else(|| panic!("no recipe for {props} in {states:?}"))
}

fn has_attribute(recipe: &Value, name: &str) -> bool {
    recipe["attrs"].get(name).is_some_and(|v| !v.is_null() && *v != json!(false))
}

#[test]
fn each_variant_gets_a_recipe_with_its_properties_attributes_and_pseudo_class() {
    // covers: E6-7.6
    let mut s = Session::new();
    let rows = [
        "Style=Primary, State=Default",
        "Style=Primary, State=Hover",
        "Style=Secondary, State=Pressed",
        "Style=Secondary, State=Disabled",
        "Style=Primary, State=Focus",
    ];
    let set = state_set(&mut s, "Control", &rows);
    let frame = idv(&ex(&mut s, "frame.create", json!({"x": 900, "y": 0, "width": 50, "height": 50, "name": "Card"})));
    let plain = idv(&ex(&mut s, "component.create", json!({"ids": [frame]})));
    let v = ex(&mut s, "export.lit", json!({}));
    let entries = v["components"].as_array().unwrap();
    let entry = entries.iter().find(|c| c["id"] == json!(set)).unwrap_or_else(|| panic!("no entry for the set: {v}"));
    let states = entry["states"].as_array().unwrap_or_else(|| panic!("states is a list: {entry}"));
    assert_eq!(states.len(), rows.len(), "one recipe per variant: {states:?}");
    let variants = s.doc().unwrap().node(set).unwrap().children.clone();
    let ids: Vec<&Value> = states.iter().map(|r| &r["id"]).collect();
    assert!(variants.iter().all(|k| ids.contains(&&json!(k))), "every recipe is a variant of the set: {states:?}");

    let default = recipe(states, &json!({"Style": "Primary", "State": "Default"}));
    assert!(default["pseudo"].is_null() && !has_attribute(default, "disabled") && !has_attribute(default, "state"), "{default}");
    let hover = recipe(states, &json!({"Style": "Primary", "State": "Hover"}));
    assert_eq!(hover["pseudo"], "hover", "{hover}");
    assert!(!has_attribute(hover, "state"), "a pseudo-class state sets no attribute: {hover}");
    let pressed = recipe(states, &json!({"Style": "Secondary", "State": "Pressed"}));
    assert_eq!((pressed["pseudo"].as_str(), pressed["attrs"]["style-variant"].as_str()), (Some("active"), Some("Secondary")), "{pressed}");
    let disabled = recipe(states, &json!({"Style": "Secondary", "State": "Disabled"}));
    assert!(disabled["pseudo"].is_null() && has_attribute(disabled, "disabled"), "{disabled}");
    assert_eq!(disabled["attrs"]["style-variant"], "Secondary", "{disabled}");
    let focus = recipe(states, &json!({"Style": "Primary", "State": "Focus"}));
    assert!(focus["pseudo"].as_str().is_some_and(|p| p.starts_with("focus")), "{focus}");

    let card = entries.iter().find(|c| c["id"] == json!(plain)).unwrap_or_else(|| panic!("no entry for the plain component: {v}"));
    assert!(card["states"].is_array(), "{card}");
}

#[test]
fn absurd_parameters_are_refused_or_answered_but_never_panic_or_write() {
    // covers: E6-7.7
    let mut s = Session::new();
    let id = swatch(&mut s, 20, 0.0);
    let dir = Folder::new("sweep");
    let image = save(&dir, "good.png", &rendered(&s, id, 1.0));
    let baseline = json!({"id": id, "image": image});
    assert_eq!(differing(&ex(&mut s, CMD, baseline.clone())), 0, "the command exists and compares");
    let before = dir.names();
    let junk = [
        json!(null),
        json!(true),
        json!(0),
        json!(-1),
        json!(0.5),
        json!(256),
        json!(1e99),
        json!(1e308),
        json!(""),
        json!("\u{0}"),
        json!("8"),
        json!("../.."),
        json!([]),
        json!({}),
        json!([1, 2]),
        json!({"id": 1}),
        json!(dir.file("")),
    ];
    for key in ["id", "image", "tolerance", "scale"] {
        for value in &junk {
            let mut params = baseline.clone();
            params[key] = value.clone();
            let crashed = matches!(s.execute(CMD, &params), Err(EngineError::Internal { .. } | EngineError::UnknownCommand(_)));
            assert!(!crashed, "{params}: the command crashed or is missing");
        }
    }
    assert!(s.execute(CMD, &json!({})).is_err() && s.execute(CMD, &json!("junk")).is_err() && s.execute(CMD, &json!(null)).is_err());
    assert_eq!(dir.names(), before, "nothing was written");
}
