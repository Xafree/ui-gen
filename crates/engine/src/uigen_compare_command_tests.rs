//! `uigen.compare`: a variant of an imported set measured against the PNG its import kept (E5-3.1 to
//! E5-3.7). The service is a scripted backend, the originals are solid rectangles written by hand
//! in a temporary project, text masked; the command reads only `.uigen/captures/`, writes nothing.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use figcraft_doc::{Document, NodeId};
use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};

use crate::cmd::all_commands;
use crate::file_access::{self, Roots};
use crate::params::id_of;
use crate::uigen_import_support::{Bench, IMPORT, PACKAGE, Png, Scenario, button_inspect, synthetic_button, synthetic_source, variant_png};
use crate::{EngineError, REFERENCE_KEY, Result, Session, reference_of};

const COMPARE: &str = "uigen.compare";
const PLACE: &str = "uigen.placeImported";
const CAPTURES: &str = ".uigen/captures";
const LIGHT: [u8; 4] = [25, 118, 210, 255];
const DARK: [u8; 4] = [144, 202, 249, 255];
const HOVER: [u8; 4] = [21, 101, 192, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];
const TEXT_BOX: [u32; 4] = [10, 6, 44, 16];
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_SIDE: u32 = 4096;
const MAX_VARIANTS: usize = 128;

fn inter_button(style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = synthetic_button(style, size, state, theme);
    let ink = snapshot["root"]["children"][0]["style"]["color"].clone();
    let text_box = json!({"x": 30, "y": 26, "width": 44, "height": 16});
    snapshot["root"]["children"][0]["box"] = text_box.clone();
    snapshot["root"]["children"][0]["lineBox"] = text_box;
    snapshot["root"]["children"][0]["style"] =
        json!({"color": ink, "fontSize": "14px", "fontFamily": "Inter", "platformFont": "Inter", "fontWeight": "400"});
    snapshot
}

struct Lab {
    bench: Bench,
    root: PathBuf,
    set: NodeId,
    start: NodeId,
}

fn import_request(extra: &Value) -> Value {
    let mut all = json!({
        "package": PACKAGE, "component": "Button", "styles": ["solid"], "sizes": ["sm", "md"],
        "states": ["Default"], "themes": ["light"], "stage": true
    });
    if let (Some(base), Some(more)) = (all.as_object_mut(), extra.as_object()) {
        base.extend(more.clone());
    }
    all
}

fn lab_with(tag: &str, extra: &Value, png: Png) -> Lab {
    let scenario = Scenario::new(button_inspect(true, false)).png(png).snapshots(inter_button);
    let mut bench = Bench::new(tag, &[synthetic_source(false)], scenario);
    let start = bench.session.docs[0].page;
    bench.import(&import_request(extra)).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let result = event.outcome.clone().unwrap_or_else(|error| panic!("the import failed: {}", error.message));
    let set = id_of(&result["set"]).unwrap_or_else(|| panic!("no set in {result}"));
    let file = bench.session.docs[0].path.clone().unwrap();
    let root = PathBuf::from(file).parent().unwrap().to_path_buf();
    Lab { bench, root, set, start }
}

fn lab(tag: &str, extra: &Value) -> Lab {
    lab_with(tag, extra, Png::PerVariant)
}

fn document_of(lab: &Lab) -> &Document {
    lab.bench.session.docs[0].doc.as_ref()
}

fn variant_name(style: &str, size: &str, state: &str) -> String {
    format!("Style={style}, Size={size}, State={state}")
}

fn variant_in(lab: &Lab, set: NodeId, size: &str, state: &str) -> NodeId {
    let doc = document_of(lab);
    let wanted = variant_name("solid", size, state);
    let found = doc.node(set).unwrap().children.iter().copied().find(|id| doc.node(*id).unwrap().name == wanted);
    found.unwrap_or_else(|| panic!("no variant {wanted:?}"))
}

fn encode(image: &RgbaImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).unwrap();
    bytes
}

fn original_png(width: u32, height: u32, fill: [u8; 4]) -> Vec<u8> {
    let mut image = RgbaImage::from_pixel(width, height, Rgba(fill));
    let [x, y, w, h] = TEXT_BOX;
    for column in x..(x + w).min(width) {
        for row in y..(y + h).min(height) {
            image.put_pixel(column, row, Rgba([255, 255, 255, 255]));
        }
    }
    encode(&image)
}

fn write_file(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn fill_of(state: &str, theme: &str) -> [u8; 4] {
    match (state, theme) {
        ("Hover", _) => HOVER,
        (_, "dark") => DARK,
        _ => LIGHT,
    }
}

fn path_of(size: &str, state: &str, theme: &str) -> String {
    variant_png("solid", size, &state.to_lowercase(), theme)
}

fn paint_all(lab: &Lab) {
    let doc = document_of(lab);
    for id in &doc.node(lab.set).unwrap().children {
        let reference = reference_of(doc, *id).unwrap_or_else(|| panic!("no reference on {id:?}"));
        for (theme, found) in &reference.themes {
            let png = found.png.as_deref().unwrap_or_else(|| panic!("no png for {theme}"));
            write_file(&lab.root, png, &original_png(found.width, found.height, fill_of(&reference.state, theme)));
        }
    }
}

fn paint_dark_clear(lab: &Lab) {
    let doc = document_of(lab);
    for id in &doc.node(lab.set).unwrap().children {
        let reference = reference_of(doc, *id).unwrap();
        let dark = &reference.themes["dark"];
        assert!(!dark.distinct, "precondition: the dark original equals the light one");
        write_file(&lab.root, dark.png.as_deref().unwrap(), &original_png(dark.width, dark.height, CLEAR));
    }
}

fn disk_of(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, out);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                out.insert(path, bytes);
            }
        }
    }
    let mut out = BTreeMap::new();
    collect(dir, &mut out);
    out
}

struct Frozen {
    document: String,
    disk: BTreeMap<PathBuf, Vec<u8>>,
    undo: (usize, usize),
}

fn freeze(lab: &Lab) -> Frozen {
    Frozen { document: lab.bench.document_text(0), disk: disk_of(&lab.root), undo: lab.bench.session.undo_depth() }
}

fn assert_frozen(lab: &Lab, before: &Frozen, what: &str) {
    let after = freeze(lab);
    assert!(after.document == before.document, "{what}: the document changed");
    assert!(after.disk == before.disk, "{what}: the disk changed");
    assert_eq!(after.undo, before.undo, "{what}: the undo history changed");
}

fn compare(lab: &mut Lab, params: &Value) -> Result<Value> {
    lab.bench.session.execute(COMPARE, params)
}

fn compare_ok(lab: &mut Lab, params: &Value) -> Value {
    compare(lab, params).unwrap_or_else(|error| panic!("{COMPARE}: {error}"))
}

fn compare_confined(lab: &mut Lab, params: &Value) -> Value {
    let captures = lab.root.join(CAPTURES).to_string_lossy().into_owned();
    let roots = Arc::new(Roots::new(&[captures], &[]).unwrap());
    file_access::confined(Some(&roots), || compare_ok(lab, params))
}

fn place_copy(lab: &mut Lab) -> NodeId {
    let placed = lab.bench.session.execute(PLACE, &json!({"set": lab.set})).unwrap_or_else(|error| panic!("{PLACE}: {error}"));
    id_of(&placed["set"]).unwrap_or_else(|| panic!("no set in {placed}"))
}

fn set_props(lab: &mut Lab, id: NodeId, props: &Value) {
    lab.bench.session.execute("node.set", &json!({"id": id, "props": props})).unwrap();
}

fn point_png_at(lab: &mut Lab, variant: NodeId, theme: &str, png: &str) {
    let raw = document_of(lab).node(variant).unwrap().plugin_data.get(REFERENCE_KEY).cloned().unwrap();
    let mut reference: Value = serde_json::from_str(&raw).unwrap();
    reference["themes"][theme]["png"] = json!(png);
    set_props(lab, variant, &json!({"pluginData": {REFERENCE_KEY: reference.to_string()}}));
}

fn rows(result: &Value) -> &[Value] {
    result["rows"].as_array().map_or(&[], Vec::as_slice)
}

fn row_of<'a>(result: &'a Value, size: &str, state: &str) -> &'a Value {
    let wanted = variant_name("solid", size, state);
    rows(result).iter().find(|row| row["variant"] == json!(wanted)).unwrap_or_else(|| panic!("no row {wanted:?} in {result}"))
}

fn status(row: &Value) -> &str {
    row["status"].as_str().unwrap_or_default()
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(f64::NAN)
}

fn has_reason(row: &Value) -> bool {
    row["reason"].as_str().is_some_and(|reason| !reason.is_empty())
}

fn assert_measured_ok(row: &Value) {
    assert_eq!(status(row), "ok", "{row}");
    assert!(number(&row["shapes"]["percent"]) < 1.0, "{row}");
    assert_eq!(row["shapes"]["verdict"], json!("ok"), "{row}");
    assert!(number(&row["shapes"]["total"]) > 0.0, "{row}");
    assert_eq!(row["text"]["verdict"], json!("ok"), "{row}");
    assert!(row["text"]["pixelPercent"].is_number(), "{row}");
    assert!(number(&row["text"]["maxDx"]).abs() <= 1.0, "{row}");
}

fn assert_size(row: &Value, side: &str, width: f64, height: f64) {
    assert!((number(&row[side]["width"]) - width).abs() < 1e-6, "{side} width in {row}");
    assert!((number(&row[side]["height"]) - height).abs() < 1e-6, "{side} height in {row}");
}

fn assert_summary(result: &Value, ok: u64, ko: u64, missing: u64, not_comparable: u64) {
    let summary = &result["summary"];
    let counts = [&summary["ok"], &summary["ko"], &summary["missing"], &summary["notComparable"]].map(|count| count.as_u64());
    assert_eq!(counts, [Some(ok), Some(ko), Some(missing), Some(not_comparable)], "{summary}");
}

fn assert_refused(lab: &mut Lab, what: &str, params: &Value) {
    let before = freeze(lab);
    let outcome = compare(lab, params);
    assert!(matches!(outcome, Err(EngineError::BadParams { .. })), "{what}: {outcome:?}");
    assert_frozen(lab, &before, what);
}

// covers: E5-3.1
#[test]
fn every_variant_is_measured_against_its_png_and_nothing_is_written() {
    let mut lab = lab("compare-nominal", &json!({}));
    let set = lab.set;
    paint_all(&lab);
    let before = freeze(&lab);

    let result = compare_ok(&mut lab, &json!({"set": set}));

    assert_eq!(id_of(&result["set"]), Some(lab.set), "{result}");
    assert_eq!(result["name"], json!("Button"), "{result}");
    assert_eq!(result["source"], json!({"package": PACKAGE, "version": "1.0.0", "component": "Button"}), "{result}");
    assert_eq!(result["theme"], json!("light"), "{result}");
    assert_eq!(rows(&result).len(), 2, "{result}");
    for (size, width, height) in [("sm", 80.0, 28.0), ("md", 100.0, 36.0)] {
        let row = row_of(&result, size, "Default");
        assert_measured_ok(row);
        assert_eq!((row["style"].as_str(), row["size"].as_str()), (Some("solid"), Some(size)), "{row}");
        assert_size(row, "original", width, height);
        assert_size(row, "library", width, height);
    }
    assert_summary(&result, 2, 0, 0, 0);
    assert_eq!(result["summary"]["compared"], json!(2), "{result}");
    assert_frozen(&lab, &before, COMPARE);
}

// covers: E5-3.1
#[test]
fn the_variants_param_keeps_the_named_rows_only() {
    let mut lab = lab("compare-variants", &json!({}));
    let set = lab.set;
    paint_all(&lab);

    let result = compare_ok(&mut lab, &json!({"set": set, "variants": [variant_name("solid", "md", "Default")]}));

    assert_eq!(rows(&result).len(), 1, "{result}");
    assert_measured_ok(row_of(&result, "md", "Default"));
    assert_summary(&result, 1, 0, 0, 0);
}

// covers: E5-3.2
#[test]
fn the_copy_in_the_library_compares_like_the_import_and_a_new_colour_makes_its_row_ko() {
    let mut lab = lab("compare-copy", &json!({}));
    paint_all(&lab);
    let copy = place_copy(&mut lab);

    let same = compare_ok(&mut lab, &json!({"set": copy}));

    assert_eq!(rows(&same).len(), 2, "{same}");
    assert_measured_ok(row_of(&same, "sm", "Default"));
    assert_measured_ok(row_of(&same, "md", "Default"));

    let button = variant_in(&lab, copy, "md", "Default");
    set_props(&mut lab, button, &json!({"fill": "#ff0000"}));
    let before = freeze(&lab);

    let changed = compare_ok(&mut lab, &json!({"set": copy}));

    let row = row_of(&changed, "md", "Default");
    assert_eq!(status(row), "ko", "{row}");
    assert!(number(&row["shapes"]["differing"]) > 0.0, "{row}");
    assert_eq!(row["shapes"]["verdict"], json!("ko"), "{row}");
    assert_measured_ok(row_of(&changed, "sm", "Default"));
    assert_summary(&changed, 1, 1, 0, 0);
    assert_frozen(&lab, &before, COMPARE);
}

// covers: E5-3.3
#[test]
fn an_absent_or_unreadable_png_is_a_missing_reference_and_the_other_rows_are_still_measured() {
    let mut lab = lab("compare-missing", &json!({"states": ["Default", "Hover"]}));
    let set = lab.set;
    paint_all(&lab);
    std::fs::remove_file(lab.root.join(path_of("sm", "Default", "light"))).unwrap();
    write_file(&lab.root, &path_of("md", "Default", "light"), b"this is not a png");
    let before = freeze(&lab);

    let result = compare_ok(&mut lab, &json!({"set": set}));

    assert_eq!(rows(&result).len(), 4, "{result}");
    for size in ["sm", "md"] {
        let row = row_of(&result, size, "Default");
        assert_eq!(status(row), "missingReference", "{row}");
        assert!(has_reason(row), "{row}");
        assert_measured_ok(row_of(&result, size, "Hover"));
    }
    assert_summary(&result, 2, 0, 2, 0);
    assert_frozen(&lab, &before, COMPARE);
}

// covers: E5-3.3
#[test]
fn a_png_over_8_mib_or_over_4096_pixels_is_missing_with_the_reason_original_too_large() {
    let mut lab = lab("compare-large", &json!({"states": ["Default", "Hover"]}));
    let set = lab.set;
    paint_all(&lab);
    let wide = encode(&RgbaImage::from_pixel(MAX_SIDE + 1, MAX_SIDE + 1, Rgba(LIGHT)));
    write_file(&lab.root, &path_of("sm", "Default", "light"), &wide);
    write_file(&lab.root, &path_of("md", "Default", "light"), &vec![0; MAX_BYTES + 1]);

    let result = compare_ok(&mut lab, &json!({"set": set}));

    for size in ["sm", "md"] {
        let row = row_of(&result, size, "Default");
        assert_eq!(status(row), "missingReference", "{row}");
        assert!(row["reason"].as_str().is_some_and(|reason| reason.contains("original too large")), "{row}");
        assert_measured_ok(row_of(&result, size, "Hover"));
    }
    assert_summary(&result, 2, 0, 2, 0);
}

// covers: E5-3.3
#[test]
fn a_reference_without_png_is_a_missing_reference_not_a_command_error() {
    let mut lab = lab_with("compare-no-png", &json!({}), Png::Missing);
    let set = lab.set;
    let before = freeze(&lab);

    let result = compare_ok(&mut lab, &json!({"set": set}));

    assert_eq!(rows(&result).len(), 2, "{result}");
    for size in ["sm", "md"] {
        let row = row_of(&result, size, "Default");
        assert_eq!(status(row), "missingReference", "{row}");
        assert!(has_reason(row), "{row}");
    }
    assert_summary(&result, 0, 0, 2, 0);
    assert_frozen(&lab, &before, COMPARE);
}

// covers: E5-3.4
#[test]
fn a_new_import_fills_the_dark_value_an_existing_variable_lacks() {
    let themes = json!({"themes": ["light", "dark"]});
    let mut lab = lab("compare-dark-gap", &themes);
    let doc = lab.bench.session.doc_mut().unwrap();
    let dark = doc.variables.collections.iter().flat_map(|collection| &collection.modes).find(|mode| mode.name == "Dark").unwrap().id.clone();
    let expected: Vec<_> = doc.variables.variables.iter().map(|variable| (variable.id.clone(), variable.values[&dark].clone())).collect();
    assert!(!expected.is_empty());
    for variable in &mut doc.variables.variables {
        variable.values.remove(&dark);
    }

    lab.bench.import(&import_request(&themes)).unwrap();
    let events = lab.bench.pump_to_end();
    assert!(events.iter().any(|event| event.command == IMPORT && event.outcome.is_ok()), "{events:?}");

    let doc = document_of(&lab);
    for (id, value) in expected {
        let variable = doc.variables.variables.iter().find(|variable| variable.id == id).unwrap();
        assert_eq!(variable.values.get(&dark), Some(&value), "{}", variable.name);
    }
}

#[test]
fn the_dark_theme_is_measured_in_the_dark_mode_of_the_collection() {
    let mut lab = lab("compare-dark", &json!({"themes": ["light", "dark"]}));
    let set = lab.set;
    paint_all(&lab);
    write_file(&lab.root, &path_of("md", "Default", "dark"), &original_png(100, 36, LIGHT));
    let before = freeze(&lab);

    let result = compare_ok(&mut lab, &json!({"set": set, "theme": "dark"}));

    assert_eq!(result["theme"], json!("dark"), "{result}");
    let measured = row_of(&result, "sm", "Default");
    assert_eq!(status(measured), "ok", "{measured}");
    assert!(number(&measured["shapes"]["percent"]) < 1.0, "{measured}");
    let light_original = row_of(&result, "md", "Default");
    assert_eq!(status(light_original), "ko", "the dark render is not the light one: {light_original}");
    assert_summary(&result, 1, 1, 0, 0);
    assert_frozen(&lab, &before, COMPARE);
}

// covers: E5-3.4
#[test]
fn a_dark_theme_that_is_not_distinct_is_not_comparable_with_a_reason() {
    let mut lab = lab("compare-dark-same", &json!({"styles": ["outline"], "themes": ["light", "dark"]}));
    let set = lab.set;
    paint_dark_clear(&lab);
    let before = freeze(&lab);

    let result = compare_ok(&mut lab, &json!({"set": set, "theme": "dark"}));

    assert_eq!(rows(&result).len(), 2, "{result}");
    for row in rows(&result) {
        assert_eq!(status(row), "notComparable", "{row}");
        assert!(has_reason(row), "{row}");
    }
    assert_summary(&result, 0, 0, 0, 2);
    assert_frozen(&lab, &before, COMPARE);
}

// covers: E5-3.4
#[test]
fn a_variant_that_looks_the_same_in_dark_is_measured_when_its_set_follows_the_colour_scheme() {
    let mut lab = lab("compare-dark-partly", &json!({"styles": ["outline"], "themes": ["light", "dark"]}));
    let set = lab.set;
    paint_dark_clear(&lab);
    let follows = document_of(&lab).node(set).unwrap().children[0];
    let raw = document_of(&lab).node(follows).unwrap().plugin_data.get(REFERENCE_KEY).cloned().unwrap();
    let mut reference: Value = serde_json::from_str(&raw).unwrap();
    reference["themes"]["dark"]["distinct"] = json!(true);
    set_props(&mut lab, follows, &json!({"pluginData": {REFERENCE_KEY: reference.to_string()}}));

    let result = compare_ok(&mut lab, &json!({"set": set, "theme": "dark"}));

    assert_eq!(rows(&result).len(), 2, "{result}");
    for row in rows(&result) {
        assert_ne!(status(row), "notComparable", "{row}");
    }
}

// covers: E5-3.4
#[test]
fn an_unknown_theme_is_refused() {
    let mut lab = lab("compare-theme", &json!({"themes": ["light", "dark"]}));
    let set = lab.set;
    paint_all(&lab);

    for theme in [json!("sepia"), json!(""), json!("Dark"), json!(5)] {
        assert_refused(&mut lab, &format!("theme {theme}"), &json!({"set": set, "theme": theme}));
    }
}

// covers: E5-3.5
#[test]
fn a_variant_of_another_size_than_its_original_is_a_size_mismatch_with_both_sizes() {
    let mut lab = lab("compare-size", &json!({}));
    paint_all(&lab);
    let copy = place_copy(&mut lab);
    let button = variant_in(&lab, copy, "md", "Default");
    set_props(&mut lab, button, &json!({"width": 120, "height": 40, "fill": "#ff0000"}));
    let before = freeze(&lab);

    let result = compare_ok(&mut lab, &json!({"set": copy}));

    let row = row_of(&result, "md", "Default");
    assert_eq!(status(row), "sizeMismatch", "{row}");
    assert_size(row, "original", 100.0, 36.0);
    assert_size(row, "library", 120.0, 40.0);
    assert!(number(&row["shapes"]["total"]) > 0.0, "the shapes are still measured: {row}");
    assert!(number(&row["shapes"]["differing"]) > 0.0, "{row}");
    assert_measured_ok(row_of(&result, "sm", "Default"));
    assert_frozen(&lab, &before, COMPARE);
}

// covers: E5-3.6
#[test]
fn outside_a_project_the_comparison_is_refused() {
    let mut session = Session::empty();
    for params in [json!({"set": 1}), json!({})] {
        let outcome = session.execute(COMPARE, &params);
        assert!(matches!(outcome, Err(EngineError::BadParams { .. })), "{params}: {outcome:?}");
    }
}

// covers: E5-3.6
#[test]
fn a_missing_set_a_set_that_is_not_one_or_without_source_and_unknown_variants_are_refused() {
    let mut lab = lab("compare-refused", &json!({}));
    paint_all(&lab);
    let copy = place_copy(&mut lab);
    set_props(&mut lab, copy, &json!({"pluginData": {"uigen.source": null}}));
    let button = variant_in(&lab, lab.set, "md", "Default");
    let known = variant_name("solid", "md", "Default");
    lab.bench.session.execute("page.select", &json!({"id": lab.start})).unwrap();

    let cases = [
        ("no set on the current page", json!({})),
        ("a page", json!({"set": lab.start})),
        ("a variant", json!({"set": button})),
        ("a set without uigen.source", json!({"set": copy})),
        ("an unknown layer", json!({"set": 99_999})),
        ("an unknown variant", json!({"set": lab.set, "variants": ["Nope"]})),
        ("a known and an unknown variant", json!({"set": lab.set, "variants": [known, "Nope"]})),
    ];
    for (what, params) in &cases {
        assert_refused(&mut lab, what, params);
    }
}

// covers: E5-3.6
#[test]
fn a_set_of_more_than_128_variants_is_refused() {
    let mut lab = lab("compare-many", &json!({}));
    let copy = place_copy(&mut lab);
    while document_of(&lab).node(copy).unwrap().children.len() <= MAX_VARIANTS {
        let variants = document_of(&lab).node(copy).unwrap().children.clone();
        lab.bench.session.execute("edit.duplicate", &json!({"ids": variants})).unwrap();
    }

    assert_refused(&mut lab, "more than 128 variants", &json!({"set": copy}));
}

// covers: E5-3.6
#[test]
fn a_png_outside_the_captures_folder_is_ignored_and_its_row_is_missing() {
    let mut lab = lab("compare-outside", &json!({}));
    paint_all(&lab);
    write_file(&lab.root, "shots/shot.png", &original_png(100, 36, LIGHT));
    let copy = place_copy(&mut lab);
    let button = variant_in(&lab, copy, "md", "Default");
    point_png_at(&mut lab, button, "light", "shots/shot.png");

    let result = compare_ok(&mut lab, &json!({"set": copy}));

    let row = row_of(&result, "md", "Default");
    assert_eq!(status(row), "missingReference", "{row}");
    assert_measured_ok(row_of(&result, "sm", "Default"));
    assert_summary(&result, 1, 0, 1, 0);
}

fn junk_params(set: NodeId) -> Vec<Value> {
    vec![
        json!(null),
        json!([1, 2, 3]),
        json!({"set": "x"}),
        json!({"set": -1}),
        json!({"set": 1e308}),
        json!({"set": set, "theme": 5}),
        json!({"set": set, "theme": "\u{0}"}),
        json!({"set": set, "variants": "x"}),
        json!({"set": set, "variants": [1, null, {"x": 1}]}),
        json!({"set": set, "variants": Value::Array(vec![json!("a"); 5000])}),
    ]
}

// covers: E5-3.6
#[test]
fn the_command_is_in_the_registry_and_survives_the_junk_parameter_sweep() {
    assert!(all_commands().iter().any(|spec| spec.id == COMPARE), "{COMPARE} is not registered");
    let mut lab = lab("compare-junk", &json!({}));
    paint_all(&lab);
    let before = freeze(&lab);

    for junk in junk_params(lab.set) {
        let outcome = compare(&mut lab, &junk);
        assert!(!matches!(outcome, Err(EngineError::Internal { .. }) | Err(EngineError::UnknownCommand(_))), "{junk}: {outcome:?}");
    }

    assert_frozen(&lab, &before, COMPARE);
}

#[cfg(unix)]
fn link_dir(link: &Path, target: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn link_dir(link: &Path, target: &Path) -> bool {
    std::os::windows::fs::symlink_dir(target, link).is_ok()
        || std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(link).arg(target).output().is_ok_and(|done| done.status.success())
}

// covers: E5-3.7
#[test]
fn a_png_reached_through_a_link_that_leaves_the_captures_folder_is_never_read() {
    let mut lab = lab("compare-link", &json!({}));
    paint_all(&lab);
    write_file(&lab.root, "elsewhere/shot.png", &original_png(100, 36, LIGHT));
    let evil = lab.root.join(CAPTURES).join("evil");
    assert!(link_dir(&evil, &lab.root.join("elsewhere")), "the system refused to make a link");
    assert!(evil.join("shot.png").is_file(), "precondition: the link reaches the image");
    let copy = place_copy(&mut lab);
    let button = variant_in(&lab, copy, "md", "Default");
    point_png_at(&mut lab, button, "light", ".uigen/captures/evil/shot.png");

    let result = compare_confined(&mut lab, &json!({"set": copy}));

    let row = row_of(&result, "md", "Default");
    assert_eq!(status(row), "missingReference", "{row}");
    assert_measured_ok(row_of(&result, "sm", "Default"));
    assert_summary(&result, 1, 0, 1, 0);
}

// covers: E5-3.7
#[test]
fn a_reference_written_with_dot_dot_segments_is_never_read() {
    let mut lab = lab("compare-dotdot", &json!({}));
    paint_all(&lab);
    write_file(&lab.root, "outside.png", &original_png(100, 36, LIGHT));
    let copy = place_copy(&mut lab);
    let malicious = [("md", ".uigen/captures/../../outside.png"), ("sm", ".uigen/captures/..\\..\\outside.png")];
    for (size, png) in malicious {
        let button = variant_in(&lab, copy, size, "Default");
        point_png_at(&mut lab, button, "light", png);
    }

    let result = compare_confined(&mut lab, &json!({"set": copy}));

    for (size, _) in malicious {
        let row = row_of(&result, size, "Default");
        assert_eq!(status(row), "missingReference", "{row}");
    }
    assert_summary(&result, 0, 0, 2, 0);
}
