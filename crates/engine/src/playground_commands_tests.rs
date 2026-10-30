//! The playground settings commands (E7-5.1 to E7-5.7): `playground.set` and its siblings push the
//! token overrides to the page through `playground.apply` without touching the document, keep their
//! own history, switch theme, and `playground.saveTheme` writes the settings into a collection as
//! one undo step. The capture service is a scripted fake backend.

use figcraft_color::Color;
use figcraft_doc::{Document, VariableValue};
use serde_json::{Value, json};

use crate::EngineError;
use crate::codegen::LitNames;
use crate::lit_export::{TokenOptions, tokens_css};
use crate::playground::{Settings, Status, Template, overrides, overrides_css};
use crate::playground_support::{APPLIED_THEME, Bench, Library};
use crate::playground_targets_tests::color;

const APPLY: &str = "playground.apply";
const PREFIX: &str = "ui";
const OTHER: &str = "Autre";
const NEUTRAL: &str = "Neutre";
const SAVED: &str = "Mon thème";
const ACCENT: &str = "couleur/primaire";
const OTHER_ACCENT: &str = "couleur/accent";
const BLUE: &str = "#2563eb";
const GREEN: &str = "#16a34a";
const LIGHT_SELECTOR: &str = ":root[data-theme=\"light\"]";

fn must(bench: &mut Bench, command: &str, params: Value) -> Value {
    match bench.run(command, &params) {
        Ok(value) => value,
        Err(error) => panic!("{command} {params}: {error}"),
    }
}

fn message_of(result: crate::Result<Value>) -> String {
    match result {
        Err(error) => error.to_string(),
        Ok(value) => panic!("expected a refusal, got {value}"),
    }
}

fn is_bad(result: &crate::Result<Value>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

fn is_hex(value: &Value, hex: &str) -> bool {
    value.as_str().is_some_and(|text| text.eq_ignore_ascii_case(hex))
}

fn ready(tag: &str) -> Bench {
    Bench::ready(tag)
}

fn ready_with_other_themes(tag: &str) -> Bench {
    let mut bench = Bench::new(tag, Library::Full);
    must(&mut bench, "variable.createCollection", json!({"name": OTHER, "modes": ["Light", "Dark"]}));
    must(
        &mut bench,
        "variable.create",
        json!({"collection": OTHER, "name": OTHER_ACCENT, "type": "color", "values": {"Light": "#00aa00", "Dark": "#33ff66"}}),
    );
    must(&mut bench, "variable.createCollection", json!({"name": NEUTRAL, "modes": ["Light", "Dark"]}));
    must(
        &mut bench,
        "variable.create",
        json!({"collection": NEUTRAL, "name": "couleur/fond", "type": "color", "values": {"Light": "#f0f0f0", "Dark": "#202020"}}),
    );
    must(&mut bench, "playground.open", json!({"template": "dashboard"}));
    bench.pump();
    assert!(matches!(bench.session.playground_status(), Status::Ready), "the scripted open must leave the playground ready");
    bench
}

fn accent_settings(hex: &str) -> Settings {
    Settings { accent: Some(color(hex)), ..Settings::default() }
}

fn expected_css(bench: &Bench, collection: &str, settings: &Settings) -> String {
    let doc = bench.session.doc().unwrap();
    let theme = doc.variables.collection(collection).unwrap_or_else(|| panic!("no collection {collection}"));
    match overrides(doc, theme, &LitNames::new(doc, PREFIX), settings) {
        Ok(found) => overrides_css(&found),
        Err(error) => panic!("overrides refused: {error}"),
    }
}

fn expected_tokens(bench: &Bench, collection: &str) -> String {
    let doc = bench.session.doc().unwrap();
    let theme = doc.variables.collection(collection).unwrap_or_else(|| panic!("no collection {collection}"));
    tokens_css(doc, &[theme], &TokenOptions { prefix: PREFIX.to_string(), light: None, dark: None }).text
}

fn applies(bench: &Bench) -> Vec<Value> {
    bench.calls.playground(APPLY)
}

fn last_apply(bench: &Bench) -> Value {
    applies(bench).pop().expect("a playground.apply was submitted")
}

fn rewrites(bench: &Bench) -> String {
    last_apply(bench)["overrides"].as_str().unwrap_or_else(|| panic!("no overrides in {}", last_apply(bench))).to_string()
}

fn stored(doc: &Document, collection: &str, variable: &str, mode: &str) -> Color {
    let found = doc.variables.collection(collection).unwrap_or_else(|| panic!("no collection {collection}"));
    let mode_id = found.modes.iter().find(|m| m.name == mode).unwrap_or_else(|| panic!("no mode {mode}")).id.clone();
    let variable = doc
        .variables
        .variables
        .iter()
        .find(|v| v.collection == found.id && v.name == variable)
        .unwrap_or_else(|| panic!("no variable {variable} in {collection}"));
    match variable.values.get(&mode_id) {
        Some(VariableValue::Color(c)) => *c,
        other => panic!("{} has no colour in {mode}: {other:?}", variable.name),
    }
}

fn close(a: Color, b: Color) -> bool {
    a.to_rgba8().iter().zip(b.to_rgba8()).all(|(x, y)| x.abs_diff(y) <= 2)
}

fn written_value(css: &str, mode_selector: &str, variable_suffix: &str) -> Color {
    let block = css.split("}\n").find(|block| block.contains(mode_selector)).unwrap_or_else(|| panic!("no {mode_selector} block in {css}"));
    let line = block.lines().find(|line| line.contains(variable_suffix)).unwrap_or_else(|| panic!("no {variable_suffix} in {block}"));
    let value = line.split_once(':').map(|(_, v)| v.trim().trim_end_matches(';')).unwrap();
    Color::parse_css(value).unwrap_or_else(|| panic!("not a colour: {value}"))
}

fn names_in(doc: &Document, collection: &str) -> Vec<String> {
    let id = doc.variables.collection(collection).unwrap_or_else(|| panic!("no collection {collection}")).id.clone();
    let mut names: Vec<String> = doc.variables.variables.iter().filter(|v| v.collection == id).map(|v| v.name.clone()).collect();
    names.sort();
    names
}

fn variables_json(doc: &Document, collection: &str, except: &str) -> String {
    let id = doc.variables.collection(collection).unwrap_or_else(|| panic!("no collection {collection}")).id.clone();
    let kept: Vec<_> = doc.variables.variables.iter().filter(|v| v.collection == id && v.name != except).collect();
    serde_json::to_string(&kept).unwrap()
}

fn mode_names(doc: &Document, collection: &str) -> Vec<String> {
    doc.variables.collection(collection).unwrap_or_else(|| panic!("no collection {collection}")).modes.iter().map(|m| m.name.clone()).collect()
}

fn applied_theme(bench: &mut Bench) -> Value {
    must(bench, "project.info", json!({}))["project"]["appliedTheme"].clone()
}

// covers: E7-5.1
#[test]
fn set_pushes_the_overrides_to_the_page_and_leaves_the_document_alone() {
    let mut bench = ready("set");
    let (before, depth, pushed) = (bench.document_text(), bench.session.undo_depth(), applies(&bench).len());

    let answer = must(&mut bench, "playground.set", json!({"accent": BLUE}));

    assert!(is_hex(&answer["settings"]["accent"], BLUE), "{answer}");
    assert!(is_hex(&answer["effective"]["accent"], BLUE), "{answer}");
    assert_eq!(answer["targets"], json!({"accent": 1, "base": 3, "font": 1, "radius": 1}), "{answer}");
    let sent = applies(&bench);
    assert_eq!(sent.len(), pushed + 1, "{sent:?}");
    let css = rewrites(&bench);
    assert_eq!(css, expected_css(&bench, APPLIED_THEME, &accent_settings(BLUE)));
    assert!(css.contains(LIGHT_SELECTOR), "{css}");
    assert_eq!(bench.document_text(), before, "the settings changed the document");
    assert_eq!(bench.session.undo_depth(), depth);
    assert_eq!(must(&mut bench, "edit.undo", json!({}))["undone"], json!(false));
}

// covers: E7-5.1
#[test]
fn set_with_a_bad_value_is_refused_and_pushes_nothing() {
    let mut bench = ready("set-bad");
    let (before, pushed) = (bench.document_text(), applies(&bench).len());
    let refused = [
        json!({"accent": "red"}),
        json!({"accent": "#12345"}),
        json!({"accent": 5}),
        json!({"base": {"hue": 400, "tint": 0.5}}),
        json!({"base": {"hue": 10, "tint": 2}}),
        json!({"base": {"hue": "x", "tint": 0.5}}),
        json!({"font": "a;b"}),
        json!({"font": "x".repeat(65)}),
        json!({"radius": "huge"}),
        json!({"radiusShape": "blob"}),
        json!({"mode": "sepia"}),
        json!({"theme": 5}),
        json!({"commit": "yes"}),
        json!([1, 2]),
    ];
    for params in refused {
        let result = bench.run("playground.set", &params);
        assert!(is_bad(&result), "{params}: {result:?}");
    }
    assert_eq!(applies(&bench).len(), pushed, "a refused setting was pushed");
    assert_eq!(bench.document_text(), before);
    assert_eq!(bench.state()["canUndo"], json!(false));
}

// covers: E7-5.2
#[test]
fn a_preview_adds_no_step_and_a_commit_adds_one() {
    let mut bench = ready("steps");
    let committed = must(&mut bench, "playground.set", json!({"accent": BLUE}));
    assert_eq!((committed["canUndo"].clone(), committed["canRedo"].clone()), (json!(true), json!(false)), "{committed}");

    let previewed = must(&mut bench, "playground.set", json!({"accent": GREEN, "commit": false}));
    assert!(is_hex(&previewed["settings"]["accent"], GREEN), "{previewed}");
    assert_eq!(rewrites(&bench), expected_css(&bench, APPLIED_THEME, &accent_settings(GREEN)), "a preview is pushed");

    let back = must(&mut bench, "playground.undo", json!({}));
    assert!(is_hex(&back["settings"]["accent"], BLUE), "the preview made a step: {back}");
    assert_eq!(rewrites(&bench), expected_css(&bench, APPLIED_THEME, &accent_settings(BLUE)));

    let start = must(&mut bench, "playground.undo", json!({}));
    assert!(start["settings"]["accent"].is_null(), "{start}");
    assert_eq!(rewrites(&bench), "", "no settings, no overrides");
    assert_eq!((start["canUndo"].clone(), start["canRedo"].clone()), (json!(false), json!(true)), "{start}");

    let again = must(&mut bench, "playground.redo", json!({}));
    assert!(is_hex(&again["settings"]["accent"], BLUE), "{again}");
    assert_eq!(rewrites(&bench), expected_css(&bench, APPLIED_THEME, &accent_settings(BLUE)));
    assert_eq!((again["canUndo"].clone(), again["canRedo"].clone()), (json!(true), json!(false)), "{again}");
}

// covers: E7-5.2
#[test]
fn revert_goes_back_to_empty_settings_as_one_step() {
    let mut bench = ready("revert");
    must(&mut bench, "playground.set", json!({"accent": BLUE}));
    must(&mut bench, "playground.set", json!({"radius": "large"}));

    let reverted = must(&mut bench, "playground.revert", json!({}));

    assert!(reverted["settings"]["accent"].is_null() && reverted["settings"]["radius"].is_null(), "{reverted}");
    assert_eq!(rewrites(&bench), "");
    assert_eq!(reverted["canUndo"], json!(true), "{reverted}");
    let undone = must(&mut bench, "playground.undo", json!({}));
    assert!(is_hex(&undone["settings"]["accent"], BLUE) && undone["settings"]["radius"] == json!("large"), "{undone}");
}

// covers: E7-5.2
#[test]
fn random_with_the_same_seed_pushes_the_same_overrides() {
    let mut bench = ready("random");
    must(&mut bench, "playground.set", json!({"mode": "dark"}));

    let first = must(&mut bench, "playground.random", json!({"seed": 7}));
    let css = rewrites(&bench);
    let second = must(&mut bench, "playground.random", json!({"seed": 7}));
    let other = must(&mut bench, "playground.random", json!({"seed": 8}));

    assert!(!css.is_empty() && css.contains(LIGHT_SELECTOR), "{css}");
    assert_eq!(first["settings"], second["settings"]);
    assert_eq!(first["settings"]["mode"], json!("dark"), "the draw changed the mode: {first}");
    assert_eq!(applies(&bench).iter().rev().nth(1).unwrap()["overrides"], json!(css));
    assert_ne!(first["settings"], other["settings"]);
    assert_ne!(rewrites(&bench), css);
}

// covers: E7-5.3
#[test]
fn choosing_another_theme_pushes_its_tokens_and_clears_the_overrides_in_one_step() {
    let mut bench = ready_with_other_themes("theme");
    must(&mut bench, "playground.set", json!({"accent": BLUE}));

    let answer = must(&mut bench, "playground.set", json!({"theme": OTHER}));

    let apply = last_apply(&bench);
    assert_eq!(apply["tokens"], json!(expected_tokens(&bench, OTHER)), "{apply}");
    assert!(apply["tokens"].as_str().is_some_and(|text| text.contains("accent") && !text.contains("primaire")), "{apply}");
    assert_eq!(apply["overrides"], json!(""), "{apply}");
    assert_eq!(answer["settings"]["theme"], json!(OTHER), "{answer}");
    assert!(answer["settings"]["accent"].is_null(), "{answer}");
    assert!(is_hex(&answer["effective"]["accent"], "#00aa00"), "{answer}");
    assert_eq!(answer["targets"], json!({"accent": 1, "base": 0, "font": 0, "radius": 0}), "{answer}");

    let back = must(&mut bench, "playground.undo", json!({}));
    assert!(back["settings"]["theme"].is_null() && is_hex(&back["settings"]["accent"], BLUE), "{back}");
    assert_eq!(last_apply(&bench)["tokens"], json!(expected_tokens(&bench, APPLIED_THEME)));
    assert_eq!(rewrites(&bench), expected_css(&bench, APPLIED_THEME, &accent_settings(BLUE)));
}

// covers: E7-5.3
#[test]
fn an_unknown_theme_is_refused_and_pushes_nothing() {
    let mut bench = ready_with_other_themes("theme-unknown");
    let (pushed, before) = (applies(&bench).len(), bench.state());

    for params in [json!({"theme": "Inconnu"}), json!({"theme": ""})] {
        let result = bench.run("playground.set", &params);
        assert!(is_bad(&result), "{params}: {result:?}");
    }

    assert_eq!(applies(&bench).len(), pushed);
    assert_eq!(bench.state()["settings"], before["settings"]);
    assert_eq!(bench.state()["canUndo"], before["canUndo"]);
}

// covers: E7-5.4
#[test]
fn mode_pushes_the_theme_of_the_page() {
    let mut bench = ready("mode");

    let dark = must(&mut bench, "playground.set", json!({"mode": "dark"}));
    assert_eq!(last_apply(&bench)["theme"], json!("dark"));
    assert_eq!(dark["settings"]["mode"], json!("dark"), "{dark}");

    must(&mut bench, "playground.set", json!({"mode": "light"}));
    assert_eq!(last_apply(&bench)["theme"], json!("light"));
}

// covers: E7-5.4
#[test]
fn a_setting_without_a_target_is_refused_naming_it_and_pushes_nothing() {
    let mut bench = ready_with_other_themes("no-target");
    must(&mut bench, "playground.set", json!({"theme": OTHER}));
    let before = bench.state();
    let refused = [(json!({"font": "Georgia"}), "font"), (json!({"radius": "large"}), "radius"), (json!({"base": {"hue": 10, "tint": 0.5}}), "base")];
    for (params, setting) in refused {
        let result = bench.run("playground.set", &params);
        assert!(is_bad(&result), "{params}: {result:?}");
        assert!(message_of(bench.run("playground.set", &params)).contains(setting), "{params}");
    }

    must(&mut bench, "playground.set", json!({"theme": NEUTRAL}));
    let pushed_after_theme = applies(&bench).len();
    let result = bench.run("playground.set", &json!({"accent": BLUE}));
    assert!(is_bad(&result), "{result:?}");
    assert!(message_of(bench.run("playground.set", &json!({"accent": BLUE}))).contains("accent"));

    assert_eq!(applies(&bench).len(), pushed_after_theme, "a refused setting was pushed");
    assert_eq!(before["settings"]["theme"], json!(OTHER));
}

// covers: E7-5.4
#[test]
fn a_closed_playground_refuses_every_settings_command() {
    let commands = [
        ("playground.set", json!({"accent": BLUE})),
        ("playground.undo", json!({})),
        ("playground.redo", json!({})),
        ("playground.revert", json!({})),
        ("playground.random", json!({"seed": 1})),
    ];
    let never_opened = Bench::new("closed-never", Library::Full);
    let mut closed_again = ready("closed-again");
    must(&mut closed_again, "playground.close", json!({}));

    for mut bench in [never_opened, closed_again] {
        let submitted = bench.calls.submitted().len();
        for (command, params) in &commands {
            let result = bench.run(command, params);
            assert!(is_bad(&result), "{command}: {result:?}");
        }
        assert_eq!(bench.calls.submitted().len(), submitted, "a closed playground got a job");
    }
}

// covers: E7-5.5
#[test]
fn save_theme_copies_the_theme_with_the_overrides_written_in_each_mode_as_one_step() {
    let mut bench = ready("save");
    must(&mut bench, "playground.set", json!({"accent": BLUE}));
    let settings = accent_settings(BLUE);
    let css = expected_css(&bench, APPLIED_THEME, &settings);
    let (before, depth) = (bench.session.doc().unwrap().clone(), bench.session.undo_depth());
    let pushed = applies(&bench).len();

    let saved = must(&mut bench, "playground.saveTheme", json!({"name": SAVED}));

    assert!(saved["collection"].as_str().is_some_and(|c| c == SAVED || bench.session.doc().unwrap().variables.collection(c).is_some()), "{saved}");
    assert_eq!(bench.session.undo_depth().0, depth.0 + 1, "one undo step");
    let doc = bench.session.doc().unwrap().clone();
    assert_eq!(mode_names(&doc, SAVED), mode_names(&doc, APPLIED_THEME));
    assert_eq!(names_in(&doc, SAVED), names_in(&doc, APPLIED_THEME));
    assert!(close(stored(&doc, SAVED, ACCENT, "Light"), written_value(&css, LIGHT_SELECTOR, "primaire")));
    assert!(close(stored(&doc, SAVED, ACCENT, "Dark"), written_value(&css, ":root[data-theme=\"dark\"]", "primaire")));
    assert_ne!(stored(&doc, SAVED, ACCENT, "Light"), stored(&doc, APPLIED_THEME, ACCENT, "Light"));
    assert_eq!(stored(&doc, SAVED, "couleur/succes", "Dark"), stored(&doc, APPLIED_THEME, "couleur/succes", "Dark"));
    assert_eq!(variables_json(&doc, APPLIED_THEME, ""), variables_json(&before, APPLIED_THEME, ""), "the chosen theme changed");
    assert_eq!(applied_theme(&mut bench), json!(APPLIED_THEME));
    assert_eq!(applies(&bench).len(), pushed, "saving pushed to the page");

    assert_eq!(must(&mut bench, "edit.undo", json!({}))["undone"], json!(true));
    assert!(bench.session.doc().unwrap().variables.collection(SAVED).is_none(), "undo left the collection");
    assert_eq!(bench.session.undo_depth().0, depth.0);
    assert_eq!(must(&mut bench, "edit.redo", json!({}))["redone"], json!(true));
    let redone = bench.session.doc().unwrap().clone();
    assert!(close(stored(&redone, SAVED, ACCENT, "Light"), stored(&doc, SAVED, ACCENT, "Light")));
}

// covers: E7-5.5
#[test]
fn save_theme_with_apply_makes_the_copy_the_applied_theme() {
    let mut bench = ready("save-apply");
    must(&mut bench, "playground.set", json!({"accent": BLUE}));

    must(&mut bench, "playground.saveTheme", json!({"name": SAVED, "apply": true}));

    assert_eq!(applied_theme(&mut bench), json!(SAVED));
    assert_eq!(bench.state()["applied"], json!(SAVED));
}

// covers: E7-5.5
#[test]
fn save_theme_copies_the_chosen_theme_not_the_applied_one() {
    let mut bench = ready_with_other_themes("save-other");
    must(&mut bench, "playground.set", json!({"theme": OTHER}));
    must(&mut bench, "playground.set", json!({"accent": BLUE}));
    let css = expected_css(&bench, OTHER, &accent_settings(BLUE));

    must(&mut bench, "playground.saveTheme", json!({"name": SAVED}));

    let doc = bench.session.doc().unwrap();
    assert_eq!(names_in(doc, SAVED), [OTHER_ACCENT]);
    assert!(close(stored(doc, SAVED, OTHER_ACCENT, "Light"), written_value(&css, LIGHT_SELECTOR, "accent")));
    assert!(close(stored(doc, SAVED, OTHER_ACCENT, "Dark"), written_value(&css, ":root[data-theme=\"dark\"]", "accent")));
}

// covers: E7-5.5
#[test]
fn save_theme_refuses_a_name_already_taken_and_leaves_the_document_identical() {
    let mut bench = ready_with_other_themes("save-taken");
    must(&mut bench, "playground.set", json!({"accent": BLUE}));
    let before = bench.document_text();

    for name in [APPLIED_THEME, OTHER, "Primitives"] {
        let result = bench.run("playground.saveTheme", &json!({"name": name}));
        assert!(is_bad(&result), "{name}: {result:?}");
        assert_eq!(bench.document_text(), before, "{name}");
    }
    for params in [json!({}), json!({"name": ""}), json!({"name": 5})] {
        let result = bench.run("playground.saveTheme", &params);
        assert!(is_bad(&result), "{params}: {result:?}");
    }
    assert_eq!(bench.document_text(), before);
}

// covers: E7-5.6
#[test]
fn save_theme_update_rewrites_the_values_of_the_chosen_collection_as_one_step() {
    let mut bench = ready("update");
    must(&mut bench, "playground.set", json!({"accent": BLUE}));
    let css = expected_css(&bench, APPLIED_THEME, &accent_settings(BLUE));
    let (before, depth) = (bench.session.doc().unwrap().clone(), bench.session.undo_depth());

    must(&mut bench, "playground.saveTheme", json!({"name": APPLIED_THEME, "update": true}));

    let doc = bench.session.doc().unwrap().clone();
    assert_eq!(doc.variables.collections.len(), before.variables.collections.len(), "update made a collection");
    assert_eq!(mode_names(&doc, APPLIED_THEME), mode_names(&before, APPLIED_THEME));
    assert_eq!(names_in(&doc, APPLIED_THEME), names_in(&before, APPLIED_THEME));
    assert!(close(stored(&doc, APPLIED_THEME, ACCENT, "Light"), written_value(&css, LIGHT_SELECTOR, "primaire")));
    assert!(close(stored(&doc, APPLIED_THEME, ACCENT, "Dark"), written_value(&css, ":root[data-theme=\"dark\"]", "primaire")));
    assert_eq!(variables_json(&doc, APPLIED_THEME, ACCENT), variables_json(&before, APPLIED_THEME, ACCENT), "other variables changed");
    assert_eq!(bench.session.undo_depth().0, depth.0 + 1, "one undo step");

    must(&mut bench, "edit.undo", json!({}));
    let undone = bench.session.doc().unwrap();
    assert_eq!(variables_json(undone, APPLIED_THEME, ""), variables_json(&before, APPLIED_THEME, ""));
    assert_eq!(bench.session.undo_depth().0, depth.0);
}

// covers: E7-5.6
#[test]
fn save_theme_update_without_settings_is_refused_and_says_so() {
    let mut bench = ready("update-empty");
    let before = bench.document_text();

    let result = bench.run("playground.saveTheme", &json!({"name": APPLIED_THEME, "update": true}));

    assert!(is_bad(&result), "{result:?}");
    assert!(message_of(bench.run("playground.saveTheme", &json!({"name": APPLIED_THEME, "update": true}))).contains("setting"));
    assert_eq!(bench.document_text(), before);
}

// covers: E7-5.7
#[test]
fn code_reports_the_overrides_the_template_and_the_notes_without_writing_anything() {
    let mut bench = ready("code");
    must(&mut bench, "playground.set", json!({"accent": BLUE}));
    let (before, submitted) = (bench.document_text(), bench.calls.submitted().len());
    let staged = std::fs::read_to_string(bench.staged("template.html")).unwrap();

    let code = must(&mut bench, "playground.code", json!({}));

    assert_eq!(code["overrides"], json!(expected_css(&bench, APPLIED_THEME, &accent_settings(BLUE))), "{code}");
    assert_eq!(code["template"], json!(bench.composed(Template::Dashboard).html), "{code}");
    assert!(code["notes"].is_array(), "{code}");
    assert_eq!(bench.document_text(), before);
    assert_eq!(bench.calls.submitted().len(), submitted, "code asked the service for something");
    assert_eq!(std::fs::read_to_string(bench.staged("template.html")).unwrap(), staged);

    must(&mut bench, "playground.revert", json!({}));
    assert_eq!(must(&mut bench, "playground.code", json!({}))["overrides"], json!(""));
}

// covers: E7-5.7
#[test]
fn the_radius_shape_goes_to_the_page_only() {
    let mut bench = ready("shape");
    must(&mut bench, "playground.set", json!({"accent": BLUE}));

    let answer = must(&mut bench, "playground.set", json!({"radiusShape": "squircle"}));

    assert_eq!(last_apply(&bench)["shape"], json!("squircle"));
    assert_eq!(answer["settings"]["radiusShape"], json!("squircle"), "{answer}");
    let code = must(&mut bench, "playground.code", json!({}));
    let css = code["overrides"].as_str().unwrap();
    assert_eq!(css, expected_css(&bench, APPLIED_THEME, &accent_settings(BLUE)));
    assert!(!css.contains("squircle") && !css.contains("corner-shape"), "{css}");
    assert!(!rewrites(&bench).contains("corner-shape"), "the shape entered the pushed overrides");

    let before = names_in(bench.session.doc().unwrap(), APPLIED_THEME);
    let css = expected_css(&bench, APPLIED_THEME, &accent_settings(BLUE));
    must(&mut bench, "playground.saveTheme", json!({"name": SAVED}));
    let doc = bench.session.doc().unwrap();
    assert_eq!(names_in(doc, SAVED), before, "the shape made a variable");
    let saved = variables_json(doc, SAVED, "");
    assert!(!saved.contains("squircle") && !saved.contains("corner"), "the shape was saved");
    assert!(close(stored(doc, SAVED, ACCENT, "Light"), written_value(&css, LIGHT_SELECTOR, "primaire")));
}
