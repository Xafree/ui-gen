//! `uigen.importComponent` keeps the original of every variant: `uigen.reference` on each variant, the
//! import options on the set, nothing for a failed import. The service is a scripted backend.

use std::collections::BTreeMap;

use figcraft_doc::{Document, NodeId};
use serde_json::{Value, json};

use crate::capture::{CaptureError, CaptureErrorKind, CaptureEvent};
use crate::params::id_of;
use crate::uigen_import_support::{
    Bench, FailingRender, IMPORT, PACKAGE, Png, Scenario, VERSION, button_inspect, service_failure, synthetic_button, synthetic_source, variant_png,
};
use crate::{EngineError, OPTIONS_KEY, REFERENCE_KEY, Result, reference_of};

const SOURCE: &str = "uigen.source";
const IDENTICAL: &str = "the dark render is identical to the light one: the library does not follow prefers-color-scheme";
const MAX_TEXTS: usize = 64;
const MAX_BYTES: usize = 16 * 1024;
const MAX_PNG: usize = 512;
const CAPTURES: &str = ".uigen/captures/";

fn params(extra: &Value) -> Value {
    let mut all =
        json!({"package": PACKAGE, "component": "Button", "styles": ["solid"], "sizes": ["md"], "states": ["Default"], "themes": ["light"]});
    if let (Some(base), Some(more)) = (all.as_object_mut(), extra.as_object()) {
        base.extend(more.clone());
    }
    all
}

fn bench(tag: &str, scenario: Scenario) -> Bench {
    Bench::new(tag, &[synthetic_source(false)], scenario)
}

fn import_outcome(bench: &mut Bench, request: &Value) -> std::result::Result<Value, CaptureError> {
    bench.import(request).unwrap();
    let events = bench.pump_to_end();
    let found: Vec<&CaptureEvent> = events.iter().filter(|event| event.command == IMPORT).collect();
    match found.as_slice() {
        [event] => event.outcome.clone(),
        other => panic!("one import event expected: {other:?}"),
    }
}

fn import_ok(bench: &mut Bench, request: &Value) -> Value {
    import_outcome(bench, request).unwrap_or_else(|error| panic!("the import failed: {}", error.message))
}

fn document_of(bench: &Bench) -> &Document {
    bench.session.docs[0].doc.as_ref()
}

fn set_of(result: &Value) -> NodeId {
    id_of(&result["set"]).unwrap_or_else(|| panic!("no set in {result}"))
}

fn warnings_of(result: &Value) -> Vec<String> {
    result["warnings"].as_array().map(|list| list.iter().filter_map(|w| w.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn variants_of(doc: &Document, set: NodeId) -> Vec<(String, NodeId)> {
    doc.node(set).unwrap().children.iter().map(|id| (doc.node(*id).unwrap().name.clone(), *id)).collect()
}

fn axes_of(name: &str) -> BTreeMap<String, String> {
    name.split(", ").filter_map(|part| part.split_once('=')).map(|(key, value)| (key.to_string(), value.to_string())).collect()
}

fn raw_of(doc: &Document, id: NodeId) -> String {
    doc.node(id).unwrap().plugin_data.get(REFERENCE_KEY).cloned().unwrap_or_else(|| panic!("no {REFERENCE_KEY} on {}", doc.node(id).unwrap().name))
}

fn faces_snapshot(style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = synthetic_button(style, size, state, theme);
    let fill = if theme == "dark" { "rgb(1, 2, 3)" } else { "rgb(4, 5, 6)" };
    snapshot["root"]["style"] = json!({"backgroundColor": fill});
    snapshot["root"]["children"] = json!([
        {
            "type": "text", "text": "Button",
            "box": {"x": 32, "y": 28, "width": 38, "height": 14},
            "lineBox": {"x": 30, "y": 26, "width": 42, "height": 18},
            "style": {"color": "rgb(255, 255, 255)", "fontSize": "14px", "fontFamily": "Synthetic Sans, Arial", "platformFont": "Synthetic Sans", "fontWeight": "500"}
        },
        {
            "type": "text", "text": "Label",
            "box": {"x": 60, "y": 28, "width": 20, "height": 14},
            "style": {"color": "rgb(255, 255, 255)", "fontSize": "20px", "fontFamily": "Synthetic Serif", "fontWeight": "bold"}
        }
    ]);
    snapshot
}

fn crowd(count: usize, style_of: fn(usize) -> Value, snapshot: &mut Value) {
    let texts: Vec<Value> = (0..count)
        .map(|index| {
            json!({
                "type": "text", "text": format!("t{index}"),
                "box": {"x": 24 + (index % 8) * 4, "y": 24, "width": 4, "height": 10},
                "style": style_of(index)
            })
        })
        .collect();
    snapshot["root"]["children"] = json!(texts);
}

fn short_style(index: usize) -> Value {
    json!({"color": "rgb(0, 0, 0)", "fontSize": format!("{}px", 10 + index)})
}

fn long_style(index: usize) -> Value {
    json!({"color": "rgb(0, 0, 0)", "fontSize": format!("{}px", 10 + index), "platformFont": format!("{index:0>180}")})
}

fn crowded_snapshot(style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = synthetic_button(style, size, state, theme);
    crowd(MAX_TEXTS + 6, short_style, &mut snapshot);
    snapshot
}

fn heavy_snapshot(style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = synthetic_button(style, size, state, theme);
    crowd(MAX_TEXTS, long_style, &mut snapshot);
    snapshot
}

fn hostile_size_snapshot(style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = synthetic_button(style, size, state, theme);
    crowd(2, |_| json!({"color": "rgb(0, 0, 0)", "fontSize": "NaNpx"}), &mut snapshot);
    snapshot
}

fn hostile_box_snapshot(style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = synthetic_button(style, size, state, theme);
    snapshot["root"]["box"] = json!({"x": -1.7e308, "y": 20, "width": 100, "height": 36});
    snapshot["root"]["children"] = json!([{
        "type": "text", "text": "Far",
        "box": {"x": 1.7e308, "y": 26, "width": 1.7e308, "height": 16},
        "style": {"color": "rgb(0, 0, 0)", "fontSize": "14px"}
    }]);
    snapshot
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn near_all(found: [f64; 4], expected: [f64; 4]) -> bool {
    found.iter().zip(expected).all(|(a, b)| near(*a, b))
}

// covers: E5-1.1
#[test]
fn every_variant_carries_the_original_of_each_theme_and_the_set_the_options_of_the_call() {
    let mut bench = bench("reference-nominal", Scenario::new(button_inspect(true, false)).png(Png::PerVariant).snapshots(faces_snapshot));
    let request = params(&json!({"styleProp": "variant", "sizes": null, "states": ["Default", "Hover"], "themes": ["light", "dark"]}));

    let result = import_ok(&mut bench, &request);

    let doc = document_of(&bench);
    let set = set_of(&result);
    let variants = variants_of(doc, set);
    assert_eq!(variants.len(), 4, "{variants:?}");
    for (name, id) in &variants {
        let axes = axes_of(name);
        let (style, size, state) = (axes["Style"].as_str(), axes["Size"].as_str(), axes["State"].as_str());
        let reference = reference_of(doc, *id).unwrap_or_else(|| panic!("no readable reference on {name}"));
        assert_eq!((reference.package.as_str(), reference.version.as_str(), reference.component.as_str()), (PACKAGE, VERSION, "Button"), "{name}");
        assert!(reference.state.eq_ignore_ascii_case(state), "{name}: state {:?}", reference.state);
        assert_eq!(reference.themes.keys().map(String::as_str).collect::<Vec<_>>(), ["dark", "light"], "{name}");

        let (width, height) = if size == "sm" { (80, 28) } else { (100, 36) };
        for theme in ["light", "dark"] {
            let found = &reference.themes[theme];
            assert_eq!(found.png.as_deref(), Some(variant_png(style, size, &state.to_lowercase(), theme).as_str()), "{name} {theme}");
            assert_eq!((found.width, found.height), (width, height), "{name} {theme}");
            assert!(found.distinct, "{name} {theme}: the dark render differs from the light one");
            assert_eq!(found.texts.len(), 2, "{name} {theme}");
            let (first, second) = (&found.texts[0], &found.texts[1]);
            assert!(near_all(first.box_, [12.0, 8.0, 38.0, 14.0]), "{name} {theme}: {:?}", first.box_);
            assert!(first.line.is_some_and(|line| near_all(line, [10.0, 6.0, 42.0, 18.0])), "{name} {theme}: {:?}", first.line);
            assert_eq!((first.family.as_str(), first.weight), ("Synthetic Sans", 500), "{name} {theme}");
            assert!(near(first.size, 14.0), "{name} {theme}: {}", first.size);
            assert!(near_all(second.box_, [40.0, 8.0, 20.0, 14.0]), "{name} {theme}: {:?}", second.box_);
            assert_eq!(second.line, None, "{name} {theme}");
            assert_eq!((second.family.as_str(), second.weight), ("Synthetic Serif", 700), "{name} {theme}");
            assert!(near(second.size, 20.0), "{name} {theme}: {}", second.size);
        }
    }

    let data = &doc.node(set).unwrap().plugin_data;
    let options: Value = serde_json::from_str(data.get(OPTIONS_KEY).unwrap_or_else(|| panic!("no {OPTIONS_KEY}: {data:?}"))).unwrap();
    let mut sizes: Vec<&str> = options["sizes"].as_array().unwrap_or_else(|| panic!("{options}")).iter().filter_map(Value::as_str).collect();
    sizes.sort_unstable();
    assert_eq!(sizes, ["md", "sm"], "{options}");
    assert_eq!(options["styleProp"], json!("variant"), "{options}");
    assert_eq!(options["sizeProp"], json!("size"), "{options}");
    assert_eq!(options["styles"], json!(["solid"]), "{options}");
    assert_eq!(options["states"], json!(["Default", "Hover"]), "{options}");
    assert_eq!(options["themes"], json!(["light", "dark"]), "{options}");
    assert_eq!(options["animations"], json!(true), "{options}");

    let expected_source = serde_json::to_string(&json!({"package": PACKAGE, "version": VERSION, "component": "Button"})).unwrap();
    assert_eq!(data.get(SOURCE), Some(&expected_source), "uigen.source is the one of lot 3");
}

// covers: E5-1.2
#[test]
fn a_dark_theme_whose_snapshot_equals_the_light_one_is_not_distinct_and_the_lot_3_warning_stays() {
    let mut same = bench("distinct-same", Scenario::new(button_inspect(true, false)).png(Png::PerVariant));
    let result = import_ok(&mut same, &params(&json!({"styles": ["outline"], "themes": ["light", "dark"]})));

    let doc = document_of(&same);
    let variants = variants_of(doc, set_of(&result));
    assert_eq!(variants.len(), 1);
    let reference = reference_of(doc, variants[0].1).unwrap();
    assert!(reference.themes["light"].distinct);
    assert!(!reference.themes["dark"].distinct);
    assert!(reference.themes["dark"].png.is_some(), "the dark original is kept even when it equals the light one");
    assert!(warnings_of(&result).iter().any(|warning| warning == IDENTICAL), "{:?}", warnings_of(&result));

    let mut differs = bench("distinct-differs", Scenario::new(button_inspect(true, false)).png(Png::PerVariant));
    let result = import_ok(&mut differs, &params(&json!({"states": ["Default", "Hover"], "themes": ["light", "dark"]})));

    let doc = document_of(&differs);
    for (name, id) in variants_of(doc, set_of(&result)) {
        let reference = reference_of(doc, id).unwrap();
        assert!(reference.themes["light"].distinct, "{name}");
        let dark_differs = name.ends_with("State=Default");
        assert_eq!(reference.themes["dark"].distinct, dark_differs, "{name}");
    }
    assert!(!warnings_of(&result).iter().any(|warning| warning == IDENTICAL), "{:?}", warnings_of(&result));
}

fn bad_pngs() -> Vec<(&'static str, String)> {
    vec![
        ("absolute", "/tmp/shot.png".to_string()),
        ("drive", "C:/captures/shot.png".to_string()),
        ("parent segment", ".uigen/captures/../../secret.png".to_string()),
        ("backslash", ".uigen/captures/a\\b.png".to_string()),
        ("outside the captures folder", "shots/shot.png".to_string()),
        ("lookalike folder", ".uigen/captures-evil/shot.png".to_string()),
        ("513 characters", format!("{CAPTURES}{}.png", "a".repeat(MAX_PNG + 1 - CAPTURES.len() - ".png".len()))),
        ("empty", String::new()),
    ]
}

// covers: E5-1.3
#[test]
fn a_png_outside_the_captures_folder_is_dropped_from_the_theme_with_a_warning_naming_the_variant() {
    for (what, path) in bad_pngs() {
        let mut bench = bench("png-bad", Scenario::new(button_inspect(true, false)).png(Png::Fixed(path)));

        let result = import_ok(&mut bench, &params(&json!({"themes": ["light", "dark"]})));

        let doc = document_of(&bench);
        let variants = variants_of(doc, set_of(&result));
        assert_eq!(variants.len(), 1, "{what}");
        let reference = reference_of(doc, variants[0].1).unwrap_or_else(|| panic!("{what}: no readable reference"));
        for theme in ["light", "dark"] {
            let found = &reference.themes[theme];
            assert_eq!(found.png, None, "{what} {theme}");
            assert_eq!((found.width, found.height), (100, 36), "{what} {theme}: the rest of the theme is kept");
            assert_eq!(found.texts.len(), 1, "{what} {theme}");
        }
        let named = warnings_of(&result).iter().any(|warning| warning.contains(&variants[0].0));
        assert!(named, "{what}: no warning names {:?} in {:?}", variants[0].0, warnings_of(&result));
    }
}

// covers: E5-1.3
#[test]
fn a_render_without_png_does_the_same_and_a_png_of_512_characters_is_kept() {
    let mut missing = bench("png-missing", Scenario::new(button_inspect(true, false)).png(Png::Missing));
    let result = import_ok(&mut missing, &params(&json!({"themes": ["light", "dark"]})));

    let doc = document_of(&missing);
    let variants = variants_of(doc, set_of(&result));
    let reference = reference_of(doc, variants[0].1).unwrap();
    assert!(reference.themes.values().all(|found| found.png.is_none() && found.width == 100), "{:?}", reference.themes.keys());
    assert!(warnings_of(&result).iter().any(|warning| warning.contains(&variants[0].0)), "{:?}", warnings_of(&result));

    let longest = format!("{CAPTURES}{}.png", "a".repeat(MAX_PNG - CAPTURES.len() - ".png".len()));
    assert_eq!(longest.len(), MAX_PNG);
    let mut edge = bench("png-512", Scenario::new(button_inspect(true, false)).png(Png::Fixed(longest.clone())));
    let result = import_ok(&mut edge, &params(&json!({})));

    let doc = document_of(&edge);
    let reference = reference_of(doc, variants_of(doc, set_of(&result))[0].1).unwrap();
    assert_eq!(reference.themes["light"].png, Some(longest));
    assert!(warnings_of(&result).is_empty(), "{:?}", warnings_of(&result));
}

// covers: E5-1.4
#[test]
fn more_than_64_texts_in_a_theme_keep_the_first_64_in_tree_order_and_warn() {
    let mut bench = bench("texts-cap", Scenario::new(button_inspect(true, false)).snapshots(crowded_snapshot));

    let result = import_ok(&mut bench, &params(&json!({})));

    let doc = document_of(&bench);
    let variants = variants_of(doc, set_of(&result));
    let reference = reference_of(doc, variants[0].1).unwrap();
    let sizes: Vec<f64> = reference.themes["light"].texts.iter().map(|text| text.size).collect();
    assert_eq!(sizes.len(), MAX_TEXTS);
    assert!(sizes.iter().zip(10..).all(|(found, expected)| near(*found, f64::from(expected))), "{sizes:?}");
    assert!(!warnings_of(&result).is_empty(), "the cut is reported");
    assert!(raw_of(doc, variants[0].1).len() <= MAX_BYTES);
}

// covers: E5-1.4
#[test]
fn the_serialized_reference_never_exceeds_16_kib_and_the_texts_are_cut_as_a_last_resort() {
    let mut bench = bench("texts-bytes", Scenario::new(button_inspect(true, false)).snapshots(heavy_snapshot));

    let result = import_ok(&mut bench, &params(&json!({"themes": ["light", "dark"]})));

    let doc = document_of(&bench);
    let variants = variants_of(doc, set_of(&result));
    let raw = raw_of(doc, variants[0].1);
    assert!(raw.len() <= MAX_BYTES, "{} bytes", raw.len());
    let reference = reference_of(doc, variants[0].1).unwrap_or_else(|| panic!("the cut reference is unreadable"));
    for (theme, found) in &reference.themes {
        assert_eq!((found.width, found.height), (100, 36), "{theme}");
        assert!(found.texts.len() <= MAX_TEXTS, "{theme}");
        assert!(
            found.texts.iter().zip(10..).all(|(text, expected)| near(text.size, f64::from(expected))),
            "{theme}: the kept texts are the first ones"
        );
    }
}

fn assert_finite(raw: &str) {
    let value: Value = serde_json::from_str(raw).unwrap_or_else(|error| panic!("{error}: {raw}"));
    let themes = value["themes"].as_object().unwrap_or_else(|| panic!("no themes in {raw}"));
    for (theme, found) in themes {
        for text in found["texts"].as_array().unwrap_or_else(|| panic!("no texts in {theme}")) {
            assert!(text["size"].as_f64().is_some_and(f64::is_finite), "{theme}: size in {text}");
            for key in ["box", "line"] {
                let numbers = text[key].as_array().into_iter().flatten();
                assert!(numbers.clone().all(|number| number.as_f64().is_some_and(f64::is_finite)), "{theme}: {key} in {text}");
            }
        }
    }
}

// covers: E5-1.4
#[test]
fn no_non_finite_number_is_written_whatever_the_snapshot_says() {
    let mut nan = bench("finite-size", Scenario::new(button_inspect(true, false)).snapshots(hostile_size_snapshot));
    let result = import_ok(&mut nan, &params(&json!({})));
    let doc = document_of(&nan);
    assert_finite(&raw_of(doc, variants_of(doc, set_of(&result))[0].1));

    let mut far = bench("finite-box", Scenario::new(button_inspect(true, false)).snapshots(hostile_box_snapshot));
    let before = far.document_text(0);
    match import_outcome(&mut far, &params(&json!({}))) {
        Ok(result) => {
            let doc = document_of(&far);
            let variants = variants_of(doc, set_of(&result));
            if doc.node(variants[0].1).unwrap().plugin_data.contains_key(REFERENCE_KEY) {
                assert_finite(&raw_of(doc, variants[0].1));
            }
        }
        Err(_) => assert_eq!(far.document_text(0), before, "a refused import writes nothing"),
    }
}

fn unreadable_references() -> Vec<(&'static str, String)> {
    vec![
        ("empty", String::new()),
        ("not json", "not json".to_string()),
        ("an array", "[]".to_string()),
        ("null", "null".to_string()),
        ("no themes", json!({"package": PACKAGE, "version": VERSION, "component": "Button", "state": "Default"}).to_string()),
        (
            "themes of the wrong type",
            json!({"package": PACKAGE, "version": VERSION, "component": "Button", "state": "Default", "themes": []}).to_string(),
        ),
        (
            "a theme without size",
            json!({"package": PACKAGE, "version": VERSION, "component": "Button", "state": "Default", "themes": {"light": {}}}).to_string(),
        ),
    ]
}

fn valid_reference() -> String {
    json!({
        "package": PACKAGE, "version": VERSION, "component": "Button", "state": "Default",
        "themes": {"light": {
            "png": ".uigen/captures/synthetic-ui/1.0.0/button.png", "width": 100, "height": 36, "distinct": true,
            "texts": [{"box": [10, 6, 40, 16], "line": null, "family": "Synthetic Sans", "size": 14, "weight": 400}]
        }}
    })
    .to_string()
}

fn reading(raw: &str) -> Option<crate::Reference> {
    let mut doc = Document::new("reading");
    let page = doc.pages[0];
    doc.node_mut(page).unwrap().plugin_data.insert(REFERENCE_KEY.to_string(), raw.to_string());
    reference_of(&doc, page)
}

// covers: E5-1.5
#[test]
fn an_unreadable_truncated_or_themeless_reference_reads_as_none_and_a_valid_one_as_itself() {
    let valid = reading(&valid_reference()).unwrap_or_else(|| panic!("the valid reference is unreadable"));
    assert_eq!((valid.package.as_str(), valid.component.as_str(), valid.state.as_str()), (PACKAGE, "Button", "Default"));
    assert_eq!(valid.themes["light"].texts.len(), 1);

    for (what, raw) in unreadable_references() {
        assert!(reading(&raw).is_none(), "{what}");
    }
    let whole = valid_reference();
    for end in 0..whole.len() {
        assert!(reading(&whole[..end]).is_none(), "truncated at {end}: {}", &whole[..end]);
    }
    let doc = Document::new("without");
    assert!(reference_of(&doc, doc.pages[0]).is_none(), "a layer without reference");
}

// covers: E5-1.5
#[test]
fn the_copy_placed_in_the_library_keeps_the_reference_of_each_variant_with_the_same_paths() {
    let mut bench = bench("reference-copy", Scenario::new(button_inspect(true, false)).png(Png::PerVariant).snapshots(faces_snapshot));
    let staged = params(&json!({"states": ["Default", "Hover"], "themes": ["light", "dark"], "stage": true}));
    let imported = import_ok(&mut bench, &staged);
    let source = set_of(&imported);

    let placed = bench.session.execute("uigen.placeImported", &json!({"set": source})).unwrap();

    let doc = document_of(&bench);
    let copy = id_of(&placed["set"]).unwrap_or_else(|| panic!("no set in {placed}"));
    assert_ne!(copy, source);
    let originals = variants_of(doc, source);
    let copies = variants_of(doc, copy);
    assert_eq!(copies.len(), originals.len());
    assert_eq!(originals.len(), 8);
    for (name, original) in &originals {
        let (_, duplicate) = copies.iter().find(|(copied, _)| copied == name).unwrap_or_else(|| panic!("no variant {name} in the copy"));
        assert_ne!(duplicate, original);
        assert_eq!(raw_of(doc, *duplicate), raw_of(doc, *original), "{name}");
        let (kept, first) = (reference_of(doc, *duplicate).unwrap(), reference_of(doc, *original).unwrap());
        for theme in ["light", "dark"] {
            assert!(kept.themes[theme].png.is_some(), "{name} {theme}");
            assert_eq!(kept.themes[theme].png, first.themes[theme].png, "{name} {theme}");
        }
    }
}

// covers: E5-1.6
#[test]
fn a_failed_or_refused_import_leaves_the_document_identical_and_writes_no_reference() {
    let failing = FailingRender {
        variant: "solid",
        size: "md",
        state: "hover",
        theme: "dark",
        error: service_failure(CaptureErrorKind::Render, "page crashed: boom"),
    };
    let request = params(&json!({"states": ["Default", "Hover"], "themes": ["light", "dark"]}));
    let mut control = bench("reference-control", Scenario::new(button_inspect(true, false)).png(Png::PerVariant).snapshots(faces_snapshot));
    let kept = import_ok(&mut control, &request);
    let control_doc = document_of(&control);
    assert!(
        variants_of(control_doc, set_of(&kept)).iter().all(|(_, id)| reference_of(control_doc, *id).is_some()),
        "the same import without the failing render keeps its references"
    );

    let scenario = Scenario::new(button_inspect(true, false)).png(Png::PerVariant).snapshots(faces_snapshot).failing(failing);
    let mut bench = bench("reference-failed", scenario);
    let before = bench.document_text(0);
    let depth = bench.session.undo_depth();

    let outcome = import_outcome(&mut bench, &request);

    let error = outcome.expect_err("the import succeeded");
    assert!(error.message.contains("page crashed: boom"), "{}", error.message);
    assert_eq!(bench.document_text(0), before);
    assert_eq!(bench.session.undo_depth(), depth);
    assert!(
        document_of(&bench).nodes().all(|node| !node.plugin_data.contains_key(REFERENCE_KEY) && !node.plugin_data.contains_key(OPTIONS_KEY)),
        "no layer keeps a reference or options"
    );

    let refused: Result<Value> = bench.import(&params(&json!({"themes": ["sepia"]})));
    assert!(matches!(refused, Err(EngineError::BadParams { .. })), "{refused:?}");
    assert_eq!(bench.document_text(0), before);
    assert_eq!(bench.session.undo_depth(), depth);
}
