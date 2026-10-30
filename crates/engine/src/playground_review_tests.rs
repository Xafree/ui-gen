//! Review fixes of the playground engine: the names a theme is written under, the aliases it
//! reaches, the commands that leave the document alone, and what waits for the page.

use serde_json::{Value, json};

use crate::capture::CaptureJob;
use crate::playground::{MAX_IN_FLIGHT, Status};
use crate::playground_support::{Bench, Library};

const TWIN: &str = "Autre";
const ACCENT: &str = "couleur/primaire";
const CITED: &str = "--ui-couleur-primaire";
const KEY: &str = "{\"type\":\"key\",\"key\":\"Enter\"}";

fn must(bench: &mut Bench, command: &str, params: Value) -> Value {
    bench.run(command, &params).unwrap_or_else(|error| panic!("{command} {params}: {error}"))
}

fn bound_button(tag: &str) -> Bench {
    let mut bench = Bench::new(tag, Library::Full);
    let id = bench.session.doc().unwrap().nodes().find(|n| n.name == "Bg").unwrap().id;
    must(&mut bench, "variable.bind", json!({"ids": [id], "field": "fill", "variable": format!("v:{ACCENT}")}));
    bench
}

fn with_twin_theme(bench: &mut Bench) {
    must(bench, "variable.createCollection", json!({"name": TWIN, "modes": ["Light", "Dark"]}));
    let values = json!({"Light": "#123456", "Dark": "#654321"});
    must(bench, "variable.create", json!({"collection": TWIN, "name": ACCENT, "type": "color", "values": values}));
}

fn open(bench: &mut Bench) {
    must(bench, "playground.open", json!({"template": "dashboard"}));
    bench.pump();
    assert!(matches!(bench.session.playground_status(), Status::Ready));
}

fn last(bench: &Bench, method: &str) -> Value {
    bench.calls.playground(method).pop().unwrap_or_else(|| panic!("no {method} was submitted"))
}

fn declares(sheet: &str, name: &str) -> bool {
    sheet.lines().any(|line| line.trim_start().starts_with(&format!("{name}:")))
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_lowercase()
}

#[test]
fn another_theme_is_written_under_the_names_the_component_cites() {
    let mut bench = bound_button("review-twin");
    with_twin_theme(&mut bench);
    open(&mut bench);
    let staged = std::fs::read_to_string(bench.staged("lib/components/ui-button.ts")).unwrap();
    assert!(staged.contains(&format!("var({CITED})")), "{staged}");

    must(&mut bench, "playground.set", json!({"theme": TWIN, "accent": "#2563eb"}));

    let apply = last(&bench, "playground.apply");
    let tokens = text(&apply["tokens"]);
    assert!(declares(&tokens, CITED) && tokens.contains("#123456"), "{tokens}");
    assert!(!tokens.contains(&format!("{CITED}-2")), "{tokens}");
    let overrides = text(&apply["overrides"]);
    assert!(declares(&overrides, CITED) && !overrides.contains(&format!("{CITED}-2")), "{overrides}");
}

#[test]
fn a_saved_theme_applied_and_reopened_keeps_the_names_the_component_cites() {
    let mut bench = bound_button("review-saved");
    open(&mut bench);
    must(&mut bench, "playground.set", json!({"accent": "#2563eb"}));
    must(&mut bench, "playground.saveTheme", json!({"name": "Copie", "apply": true}));

    must(&mut bench, "playground.refresh", json!({}));
    bench.pump();

    let sheet = std::fs::read_to_string(bench.staged("lib/tokens/tokens.css")).unwrap();
    assert!(declares(&sheet, CITED) && !sheet.contains(&format!("{CITED}-2")), "{sheet}");
}

#[test]
fn the_staging_sheet_declares_the_collections_the_aliases_reach() {
    let mut bench = Bench::new("review-alias", Library::Full);
    open(&mut bench);

    let sheet = std::fs::read_to_string(bench.staged("lib/tokens/tokens.css")).unwrap();

    assert!(declares(&sheet, "--ui-gris-500"), "{sheet}");
    let notes = bench.state()["notes"].clone();
    assert!(notes.as_array().is_some_and(|all| all.iter().all(|note| note["code"] != "token-missing")), "{notes}");
}

#[test]
fn the_state_lists_what_the_export_could_not_do() {
    let mut bench = Bench::new("review-notes", Library::Full);
    assert_eq!(bench.state()["notes"], json!([]));
    let id = bench.session.doc().unwrap().nodes().find(|n| n.name == "Bg").unwrap().id;
    must(&mut bench, "variable.bind", json!({"ids": [id], "field": "fill", "variable": "v:zebre/unique"}));
    open(&mut bench);

    let notes = bench.state()["notes"].clone();

    let missing = notes.as_array().and_then(|all| all.iter().find(|note| note["code"] == "token-missing"));
    assert!(missing.is_some_and(|note| note["message"].as_str().is_some_and(|m| m.contains("zebre"))), "{notes}");
}

#[test]
fn input_resize_and_state_leave_the_revision_and_the_journal_alone() {
    let mut bench = Bench::ready("review-revision");
    let (revision, journal) = (bench.session.ds().unwrap().revision, bench.session.journal.len());

    for i in 0..100 {
        must(&mut bench, "playground.input", json!({"events": [{"type": "move", "x": f64::from(i) + 0.5, "y": 1.5}]}));
        must(&mut bench, "playground.resize", json!({"width": 900}));
        bench.state();
        bench.pump();
    }
    must(&mut bench, "playground.set", json!({"radius": null}));

    assert_eq!(bench.session.ds().unwrap().revision, revision);
    assert_eq!(bench.session.journal.len(), journal);
}

#[test]
fn saving_a_theme_still_counts_as_an_edit() {
    let mut bench = Bench::ready("review-save-revision");
    must(&mut bench, "playground.set", json!({"accent": "#2563eb"}));
    let revision = bench.session.ds().unwrap().revision;

    must(&mut bench, "playground.saveTheme", json!({"name": "Copie"}));

    assert!(bench.session.ds().unwrap().revision > revision);
}

#[test]
fn input_waits_while_the_service_is_busy() {
    let mut bench = Bench::ready("review-input-queue");
    let job = CaptureJob::Uninstall { root: bench.root(), package: "lit".to_string() };
    bench.session.submit_capture("project.removeSource", "lit", job).unwrap();
    let key: Value = serde_json::from_str(KEY).unwrap();

    must(&mut bench, "playground.input", json!({"events": [key]}));
    must(&mut bench, "playground.input", json!({"events": [key]}));

    assert!(bench.calls.playground("playground.input").is_empty(), "input was submitted while the service was busy");
    assert_eq!(bench.state()["inputWaiting"], json!(2));

    bench.calls.release();
    bench.pump();
    bench.session.playground_poll();
    bench.pump();
    let sent = bench.calls.playground("playground.input");
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0]["events"].as_array().map(Vec::len), Some(2));
    assert_eq!(bench.state()["inputWaiting"], json!(0));
}

#[test]
fn successive_moves_wait_as_the_last_one_only() {
    let mut bench = Bench::ready("review-moves");
    let at = |x: f64| json!({"events": [{"type": "move", "x": x, "y": 1.5}]});
    for i in 0..MAX_IN_FLIGHT {
        must(&mut bench, "playground.input", at(i as f64 + 0.5));
    }
    for x in [100.5, 101.5, 102.5] {
        must(&mut bench, "playground.input", at(x));
    }
    assert_eq!(bench.calls.playground("playground.input").len(), MAX_IN_FLIGHT);
    assert_eq!(bench.state()["inputWaiting"], json!(1));

    bench.pump();

    let sent = bench.calls.playground("playground.input");
    assert_eq!(sent.len(), MAX_IN_FLIGHT + 1);
    assert_eq!(sent[MAX_IN_FLIGHT]["events"], json!([{"type": "move", "x": 102.5, "y": 1.5}]));
}

#[test]
fn applies_in_flight_are_bounded_and_the_page_takes_the_latest() {
    let mut bench = Bench::ready("review-apply-queue");
    let before = bench.calls.playground("playground.apply").len();

    for i in 0..MAX_IN_FLIGHT {
        must(&mut bench, "playground.set", json!({"font": format!("Font {i}"), "commit": false}));
    }
    for accent in ["#2563eb", "#16a34a", "#dc2626"] {
        must(&mut bench, "playground.set", json!({"accent": accent, "commit": false}));
    }

    assert_eq!(bench.calls.playground("playground.apply").len(), before + MAX_IN_FLIGHT);
    assert_eq!(bench.state()["applyWaiting"], json!(true));
    assert_eq!(text(&bench.state()["settings"]["accent"]), "#dc2626");

    bench.pump();

    let sent = bench.calls.playground("playground.apply");
    assert_eq!(sent.len(), before + MAX_IN_FLIGHT + 1);
    let latest = must(&mut bench, "playground.code", json!({}));
    assert_eq!(sent[before + MAX_IN_FLIGHT]["overrides"], latest["overrides"]);
    assert!(text(&latest["overrides"]).contains(CITED), "{latest}");
    assert_eq!(bench.state()["applyWaiting"], json!(false));
}

#[test]
fn revert_keeps_the_mode_and_the_theme_and_makes_no_step_when_nothing_is_set() {
    let mut bench = Bench::ready("review-revert");
    must(&mut bench, "playground.set", json!({"mode": "dark"}));
    let steps = bench.state()["canUndo"].clone();

    let reverted = must(&mut bench, "playground.revert", json!({}));
    assert_eq!(reverted["settings"]["mode"], json!("dark"));
    assert_eq!(reverted["canUndo"], steps);
    let undone = must(&mut bench, "playground.undo", json!({}));
    assert_eq!(undone["settings"]["mode"], json!("light"));

    must(&mut bench, "playground.set", json!({"accent": "#2563eb", "mode": "dark"}));
    let emptied = must(&mut bench, "playground.revert", json!({}));
    assert!(emptied["settings"]["accent"].is_null());
    assert_eq!(emptied["settings"]["mode"], json!("dark"));
    let back = must(&mut bench, "playground.undo", json!({}));
    assert_eq!(text(&back["settings"]["accent"]), "#2563eb");
}

#[test]
fn refreshing_keeps_the_settings_while_their_theme_exists() {
    let mut bench = Bench::ready("review-refresh");
    must(&mut bench, "playground.set", json!({"accent": "#2563eb"}));

    must(&mut bench, "playground.refresh", json!({}));
    bench.pump();

    let state = bench.state();
    assert_eq!(text(&state["settings"]["accent"]), "#2563eb", "{state}");
    assert_eq!(state["canUndo"], json!(true));
    assert!(text(&last(&bench, "playground.apply")["overrides"]).contains("--ui-couleur-primaire"));
}

#[test]
fn refreshing_drops_the_settings_of_a_theme_that_is_gone() {
    let mut bench = Bench::new("review-refresh-gone", Library::Full);
    with_twin_theme(&mut bench);
    open(&mut bench);
    must(&mut bench, "playground.set", json!({"theme": TWIN}));
    must(&mut bench, "variable.deleteCollection", json!({"collection": TWIN}));

    must(&mut bench, "playground.refresh", json!({}));
    bench.pump();

    let state = bench.state();
    assert!(state["settings"]["theme"].is_null(), "{state}");
    assert_eq!(state["canUndo"], json!(false));
}

#[test]
fn a_theme_name_with_a_control_character_or_too_long_is_refused() {
    let mut bench = Bench::ready("review-name");
    must(&mut bench, "playground.set", json!({"accent": "#2563eb"}));
    let before = bench.document_text();

    for name in ["bad\nname".to_string(), "x".repeat(65)] {
        assert!(bench.run("playground.saveTheme", &json!({"name": name})).is_err());
    }

    assert_eq!(bench.document_text(), before);
}

#[test]
fn a_saved_theme_is_in_the_menu_without_a_refresh() {
    let mut bench = Bench::ready("review-menu");
    must(&mut bench, "playground.set", json!({"accent": "#2563eb"}));

    must(&mut bench, "playground.saveTheme", json!({"name": "Copie"}));

    assert!(bench.state()["themes"].as_array().is_some_and(|names| names.contains(&json!("Copie"))));
    must(&mut bench, "playground.set", json!({"theme": "Copie"}));
}
