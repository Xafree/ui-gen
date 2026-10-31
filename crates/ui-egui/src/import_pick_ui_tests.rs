//! The "Choose your components" surface that replaces the "Import a component" window: a full
//! surface under the tab bar, entered from the Source libraries page (`Choose its components`,
//! `Browse`, `Import components`), with one card per source, a side panel "Your selection" and an
//! `Import` that runs `uigen.importComponent` for every ticked component, one after another.
//!
//! Black box: the tests read what the surface paints (texts, top to bottom) and its AccessKit
//! controls (names, toggled state, bounds). A card paints its heading `<package>` and its count
//! `<n> of <m>` as texts; a component row is a checkbox named after the component; the chips of the
//! side panel are toggle controls named `Default`, `Hover`, `Focus`, `Pressed`, `Disabled`,
//! `Light`, `Dark` and `Animations`. The service is the scripted one of `import_window_ui_tests`.

use std::collections::BTreeSet;

use egui::{Key, Modifiers};
use figcraft_engine::MAX_RENDERS;
use figcraft_engine::capture::CaptureJob;
use serde_json::{Value, json};

use crate::a11y::focused_name;
use crate::dialogs::project_settings::add_packages;
use crate::import_window_ui_tests::{Bench, IMPORT, OTHER, OTHER_VERSION, PACKAGE, VERSION, inspect_of, mentions, source};

const STYLES_ONLY: &str = "styles-ui";
const STATES: [&str; 5] = ["Default", "Hover", "Focus", "Pressed", "Disabled"];
const THEMES: [&str; 2] = ["Light", "Dark"];
const HEROUI_REACT: &str = "@heroui/react";
const HEROUI_STYLES: &str = "@heroui/styles";
const FAMILY_VERSION: &str = "3.2.7";

fn at(shown: &[String], needle: &str) -> usize {
    shown.iter().position(|text| text.contains(needle)).unwrap_or_else(|| panic!("{needle:?} is not shown: {shown:?}"))
}

fn sorted(value: &Value) -> Vec<String> {
    let mut items: Vec<String> =
        value.as_array().map(|items| items.iter().filter_map(|item| item.as_str().map(str::to_string)).collect()).unwrap_or_default();
    items.sort();
    items
}

fn strings(items: &[&str]) -> Vec<String> {
    let mut items: Vec<String> = items.iter().map(ToString::to_string).collect();
    items.sort();
    items
}

fn panel_total(flat: &str, components: usize) -> usize {
    let words: Vec<&str> = flat.split_whitespace().collect();
    words.windows(2).find_map(|pair| pair[0].parse::<usize>().ok().filter(|_| pair[1].starts_with("render"))).unwrap_or(components)
}

fn pair_open(tag: &str) -> Bench {
    let mut b = Bench::pair(tag, &["Alert", "Button", "Chip"], &["Button", "Slider"]);
    b.open(None);
    b.wait_until("both cards are listed", |b| b.has("Slider") && b.has("Alert"));
    b
}

fn tick_three(b: &mut Bench) {
    b.click("Alert");
    b.click_nth("Button", 0);
    b.click("Slider");
}

fn import_calls_since(b: &Bench, mark: usize) -> Vec<Value> {
    b.ran_since(mark, IMPORT)
}

// covers: PICK-1
// covers: PICK-2
#[test]
fn browse_and_import_components_open_the_surface_with_the_right_card_first_and_cancel_goes_back() {
    let sources = [source(PACKAGE, VERSION, false), source(OTHER, OTHER_VERSION, false), source(STYLES_ONLY, "3.0.0", false)];
    let inspects = vec![
        (PACKAGE.to_string(), inspect_of(PACKAGE, VERSION, &["Alert", "Button"], &[])),
        (OTHER.to_string(), inspect_of(OTHER, OTHER_VERSION, &["Slider", "Stepper"], &[])),
        (STYLES_ONLY.to_string(), inspect_of(STYLES_ONLY, "3.0.0", &[], &[])),
    ];
    let mut b = Bench::with("entries", &sources, inspects);
    b.open_settings();
    assert!(!b.window_open());
    assert!(b.flat().contains("Installed in this project"), "the Source libraries page is showing: {:?}", b.shown());

    b.click_nth("Browse", 1);
    b.wait_until("the cards are listed", |b| b.has("Slider") && b.has("Alert"));
    assert!(b.window_open(), "Browse opens the surface: {:?}", b.shown());
    let shown = b.shown();
    assert!(at(&shown, OTHER) < at(&shown, PACKAGE), "entered from a package, its card comes first: {shown:?}");

    b.click("Cancel");
    assert!(!b.window_open(), "Cancel closes the surface");
    assert!(b.flat().contains("Installed in this project"), "and returns to the Source libraries page: {:?}", b.shown());

    b.click("Import components");
    b.wait_until("the cards are listed", |b| b.has("Slider") && b.has("Alert"));
    assert!(b.window_open(), "Import components opens the surface");
    let shown = b.shown();
    assert!(at(&shown, PACKAGE) < at(&shown, OTHER), "without a package the sources keep the project's order: {shown:?}");
    b.click("Cancel");
    assert!(b.flat().contains("Installed in this project"));
}

// covers: PICK-1
#[test]
fn choose_its_components_after_an_install_opens_the_surface_on_that_package() {
    let mut b = Bench::with("install", &[], vec![(PACKAGE.to_string(), inspect_of(PACKAGE, VERSION, &["Alert", "Button"], &[]))]);
    b.open_settings();
    add_packages(&mut b.ui.app, PACKAGE);
    b.wait_until("the library is ready", |b| b.has("Choose its components"));
    b.click("Choose its components");
    b.wait_until("the components are listed", |b| b.has("Alert"));
    assert!(b.window_open(), "the button opens the surface: {:?}", b.shown());
    assert!(b.shows(PACKAGE) && b.has("Button"), "the card of the new library is there");
}

// covers: PICK-1
#[test]
fn the_surface_has_its_crumb_and_two_steps_and_says_nothing_about_harmonising_cost_or_duration() {
    let mut b = pair_open("chrome");
    let flat = b.flat();
    assert!(flat.contains("mylib / Import components"), "the crumb is <project> / Import components: {flat}");
    assert!(flat.contains("01 Libraries ✓"), "step one is done: {flat}");
    assert!(flat.contains("02 Components"), "step two: {flat}");
    assert!(b.has("Cancel"));
    let lower = flat.to_lowercase();
    for word in ["harmon", "cost", "duration", "minute", "token", "credit"] {
        assert!(!lower.contains(word), "nothing about {word:?} on screen: {flat}");
    }
}

// covers: PICK-2
#[test]
fn each_source_with_components_gets_a_card_with_its_count_and_a_checkbox_per_component_and_search_filters() {
    let sources = [source(PACKAGE, VERSION, false), source(OTHER, OTHER_VERSION, false), source(STYLES_ONLY, "3.0.0", false)];
    let inspects = vec![
        (PACKAGE.to_string(), inspect_of(PACKAGE, VERSION, &["Alert", "Button", "Chip"], &[])),
        (OTHER.to_string(), inspect_of(OTHER, OTHER_VERSION, &["Slider", "Stepper"], &[])),
        (STYLES_ONLY.to_string(), inspect_of(STYLES_ONLY, "3.0.0", &[], &[])),
    ];
    let mut b = Bench::with("cards", &sources, inspects);
    b.open(None);
    b.wait_until("the cards are listed", |b| b.has("Slider") && b.has("Alert"));
    let shown = b.shown();
    assert!(mentions(&shown, PACKAGE) && mentions(&shown, OTHER), "one card per source with components: {shown:?}");
    assert!(mentions(&shown, "0 of 3") && mentions(&shown, "0 of 2"), "each card counts its ticked components: {shown:?}");
    assert!(!mentions(&shown, STYLES_ONLY), "a source with no component has no card: {shown:?}");
    for name in ["Alert", "Button", "Chip", "Slider", "Stepper"] {
        assert!(!b.checked(name), "{name} is a checkbox row, unticked at first");
    }

    b.click("Alert");
    assert!(b.checked("Alert"), "a click ticks the row");
    let shown = b.shown();
    assert!(mentions(&shown, "1 of 3") && mentions(&shown, "0 of 2"), "the count follows the ticks: {shown:?}");

    b.click("Search components");
    b.type_text("chi");
    b.press(Key::Enter, Modifiers::NONE);
    assert!(b.has("Chip"), "the match stays");
    assert!(!b.has("Slider") && !b.has("Stepper") && !b.has("Alert"), "the others go: {:?}", b.shown());
}

// covers: PICK-3
#[test]
fn components_that_are_not_visual_are_not_offered_and_real_ones_stay() {
    let names = ["Button", "ThemeContext", "FormProvider", "useToggle", "useDisclosure", "ContextMenu", "UserCard"];
    let mut b = Bench::nominal("visual", &names);
    b.open(Some(PACKAGE));
    b.wait_until("the components are listed", |b| b.has("Button"));
    for kept in ["Button", "ContextMenu", "UserCard"] {
        assert!(b.has(kept), "{kept} is a real component and stays");
    }
    let shown = b.shown();
    for hidden in ["ThemeContext", "FormProvider", "useToggle", "useDisclosure"] {
        assert!(!mentions(&shown, hidden) && !b.has(hidden), "{hidden} is not offered: {shown:?}");
    }
    assert!(mentions(&shown, "0 of 3"), "the card counts only what it offers: {shown:?}");
}

// covers: PICK-4
#[test]
fn a_name_ticked_in_another_source_tags_the_unticked_row_as_already_taken() {
    let mut b = pair_open("taken");
    let shown = b.shown();
    assert!(!mentions(&shown, "Already taken in"), "no tag while nothing is ticked: {shown:?}");

    b.click_nth("Button", 0);
    assert!(b.checked_nth("Button", 0) && !b.checked_nth("Button", 1));
    let shown = b.shown();
    let tags: Vec<&String> = shown.iter().filter(|text| text.contains("Already taken in")).collect();
    assert_eq!(tags.len(), 1, "one unticked row carries the tag: {shown:?}");
    assert!(tags[0].contains(&format!("Already taken in {PACKAGE}")), "it names the package that has the name: {tags:?}");
    let tag_at = at(&shown, "Already taken in");
    assert!(tag_at > at(&shown, OTHER), "the tag is in the second card, on its Button row: {shown:?}");

    b.click_nth("Button", 0);
    assert!(!mentions(&b.shown(), "Already taken in"), "unticking frees the name");
}

// covers: PICK-5
#[test]
fn the_selection_panel_counts_live_and_offers_the_state_theme_and_animation_chips() {
    let mut b = pair_open("panel");
    let flat = b.flat();
    assert!(flat.contains("Your selection"), "{flat}");
    assert!(flat.contains("0 components"), "the count starts at zero: {flat}");
    for label in STATES.iter().chain(THEMES.iter()).chain(["Animations"].iter()) {
        assert!(b.checked(label), "{label} starts on");
    }

    b.click("Alert");
    assert!(b.flat().contains("1 component"), "{:?}", b.shown());
    b.click("Slider");
    assert!(b.flat().contains("2 components"), "the count is live: {:?}", b.shown());
    b.click("Alert");
    let flat = b.flat();
    assert!(flat.contains("1 component") && !flat.contains("2 components"), "{flat}");

    b.click("Hover");
    assert!(!b.checked("Hover"), "a chip toggles off");
    b.click("Hover");
    assert!(b.checked("Hover"), "and on");
}

// covers: PICK-5
#[test]
fn import_with_nothing_chosen_or_an_empty_group_says_why_starts_nothing_and_stays_enabled() {
    let mut b = pair_open("reasons");
    let mark = b.mark();
    let import = b.control("Import").expect("an Import button");
    assert!(!import.is_disabled(), "Import is enabled with an empty selection");
    b.click("Import");
    assert!(import_calls_since(&b, mark).is_empty(), "nothing is imported");
    assert!(mentions(&b.shown(), "Choose at least one component."), "{:?}", b.shown());

    b.click("Alert");
    let groups: [&[&str]; 2] = [&STATES, &THEMES];
    for group in groups {
        for label in group {
            b.click(label);
        }
        b.click("Import");
        assert!(import_calls_since(&b, mark).is_empty(), "nothing is imported with {group:?} all off");
        assert!(mentions(&b.shown(), "Choose at least one value in each group."), "{:?}", b.shown());
        let import = b.control("Import").expect("an Import button");
        assert!(!import.is_disabled(), "Import stays enabled");
        for label in group {
            b.click(label);
        }
    }
    let started = b.shared.lock().unwrap().submitted.iter().filter(|job| !matches!(job, CaptureJob::Inspect { .. })).count();
    assert_eq!(started, 0, "no job but the readings was submitted");

    b.click("Import");
    assert_eq!(import_calls_since(&b, mark).len(), 1, "with a choice everywhere the import starts");
}

// covers: PICK-6
#[test]
fn import_runs_each_ticked_component_one_after_another_staged_with_the_chosen_options() {
    let mut b = pair_open("sequence");
    tick_three(&mut b);
    b.click("Hover");
    b.click("Dark");
    b.hold();
    let mark = b.mark();
    b.click("Import");
    assert_eq!(import_calls_since(&b, mark).len(), 1, "only the first component is started while the service holds: {:?}", b.shown());

    for _ in 0..400 {
        let started = import_calls_since(&b, mark).len();
        let finished = b.imported().len();
        assert!(started <= finished + 1, "{started} started but {finished} finished: the imports must not overlap");
        if finished == 3 {
            break;
        }
        b.release(1);
    }
    b.release_all();
    b.wait_until("the summary is drawn", |b| b.shows("Imported 3 of 3 components"));

    let calls = import_calls_since(&b, mark);
    let order: Vec<(&str, &str)> =
        calls.iter().map(|c| (c["package"].as_str().unwrap_or_default(), c["component"].as_str().unwrap_or_default())).collect();
    assert_eq!(order, [(PACKAGE, "Alert"), (PACKAGE, "Button"), (OTHER, "Slider")], "card order, then row order");
    for call in &calls {
        assert_eq!(call["stage"], json!(true), "every import is staged: {call}");
        assert_eq!(sorted(&call["states"]), strings(&["Default", "Focus", "Pressed", "Disabled"]), "{call}");
        assert_eq!(sorted(&call["themes"]), strings(&["light"]), "{call}");
    }
    let button = &calls[1];
    assert_eq!(button["styleProp"], json!("variant"));
    assert_eq!(button["sizeProp"], json!("size"));
    assert_eq!(sorted(&button["styles"]), strings(&["outline", "solid"]), "every style the props offer");
    assert_eq!(sorted(&button["sizes"]), strings(&["md", "sm"]), "every size the props offer");

    let names: BTreeSet<String> = b.imported().into_iter().map(|(name, _)| name).collect();
    assert_eq!(names, BTreeSet::from(["Alert".to_string(), "Button".to_string(), "Slider".to_string()]));
}

// covers: PICK-6
#[test]
fn one_failed_component_is_named_the_others_go_on_and_the_summary_counts_the_successes() {
    let cause = "Playwright: timeout rendering the button";
    let mut b = pair_open("failure");
    b.fail_component("Button", cause);
    tick_three(&mut b);
    let mark = b.mark();
    b.click("Import");
    b.wait_until("the summary is drawn", |b| b.shows("Imported 2 of 3 components"));

    let shown = b.shown();
    assert!(
        shown.iter().any(|text| text.contains("Couldn't import Button: ") && text.contains(cause)),
        "the failure names the component and the cause: {shown:?}"
    );
    assert_eq!(import_calls_since(&b, mark).len(), 3, "the others went on after the failure");
    let names: BTreeSet<String> = b.imported().into_iter().map(|(name, _)| name).collect();
    assert_eq!(names, BTreeSet::from(["Alert".to_string(), "Slider".to_string()]), "only the successes are in the import page");
    assert_eq!(b.ui.app.ui.last_error, None, "the surface says it, no error banner");
}

// covers: PICK-6
// covers: PICK-7
#[test]
fn the_count_in_the_panel_and_the_progress_total_agree_from_the_first_step_to_the_last() {
    let mut b = pair_open("agree");
    tick_three(&mut b);
    let promised = panel_total(&b.flat(), 3);
    b.hold();
    b.click("Import");
    let (first, total) = b.progress().unwrap_or_else(|| panic!("the progress is on screen: {:?}", b.shown()));
    assert_eq!(total, promised, "the progress total is the count the panel gave: {:?}", b.shown());
    assert!(first <= total);

    let mut seen = BTreeSet::from([first]);
    for _ in 0..400 {
        b.release(1);
        let Some((done, shown_total)) = b.progress() else { break };
        assert_eq!(shown_total, promised, "the total never changes during the run");
        assert!(done <= shown_total, "{done} of {shown_total}");
        seen.insert(done);
    }
    assert!(seen.len() > 1, "the progress moves: {seen:?}");
    b.release_all();
    b.wait_until("the summary is drawn", |b| b.shows("Imported 3 of 3 components"));
}

// covers: PICK-5
#[test]
fn the_surface_works_from_the_keyboard_alone() {
    let mut b = Bench::nominal("keyboard", &["Alert", "Button", "Chip"]);
    crate::menus::run_action(&mut b.ui.app, "command:uigen.importWindow");
    b.settle();
    b.wait_until("the components are listed", |b| b.has("Alert"));
    b.ui.ctx.enable_accesskit();

    b.press(Key::Tab, Modifiers::NONE);
    assert!(!focused_name(&b.ui.ctx).is_empty(), "the first Tab lands on a named control of the surface");

    let tab_to = |b: &mut Bench, wanted: &str| {
        let mut visited = BTreeSet::new();
        for _ in 0..150 {
            let name = focused_name(&b.ui.ctx);
            if name == wanted {
                return;
            }
            visited.insert(name);
            b.press(Key::Tab, Modifiers::NONE);
        }
        panic!("Tab never reached {wanted:?}; visited {visited:?}");
    };

    tab_to(&mut b, "Alert");
    b.press(Key::Space, Modifiers::NONE);
    assert!(b.checked("Alert"), "Space ticks the focused row");
    tab_to(&mut b, "Hover");
    b.press(Key::Space, Modifiers::NONE);
    assert!(!b.checked("Hover"), "Space toggles the focused chip");
    tab_to(&mut b, "Import");
    let mark = b.mark();
    b.press(Key::Enter, Modifiers::NONE);
    let calls = import_calls_since(&b, mark);
    assert_eq!(calls.len(), 1, "Enter on Import starts the import");
    assert_eq!(calls[0]["component"], json!("Alert"));
}

fn over_limit_reason() -> String {
    format!("More than {MAX_RENDERS} renders: choose fewer styles, sizes, states or themes")
}

fn values(prefix: &str, count: usize) -> Vec<String> {
    (0..count).map(|index| format!("{prefix}{index}")).collect()
}

fn grid_component(name: &str, styles: usize, sizes: usize) -> Value {
    let mut props = vec![json!({"name": "label", "type": "string", "values": null})];
    if styles > 0 {
        props.push(json!({"name": "variant", "type": "enum", "values": values("style-", styles)}));
    }
    if sizes > 0 {
        props.push(json!({"name": "size", "type": "enum", "values": values("size-", sizes)}));
    }
    json!({"name": name, "props": props})
}

fn grid_open(tag: &str, grid: &[(&str, usize, usize)]) -> Bench {
    let components: Vec<Value> = grid.iter().map(|(name, styles, sizes)| grid_component(name, *styles, *sizes)).collect();
    let inspected = json!({"package": PACKAGE, "version": VERSION, "framework": "react", "components": components, "notes": []});
    let mut b = Bench::with(tag, &[source(PACKAGE, VERSION, false)], vec![(PACKAGE.to_string(), Ok(inspected))]);
    b.open(None);
    let first = grid[0].0;
    b.wait_until("the components are listed", |b| b.has(first));
    assert!(b.window_open(), "the Choose your components surface is showing: {:?}", b.shown());
    b
}

fn started_jobs(b: &Bench) -> usize {
    b.shared.lock().unwrap().submitted.iter().filter(|job| !matches!(job, CaptureJob::Inspect { .. })).count()
}

fn assert_refused_for_the_limit(b: &mut Bench, mark: usize) {
    assert!(import_calls_since(b, mark).is_empty(), "nothing is imported over the limit");
    assert_eq!(started_jobs(b), 0, "no job but the readings was submitted");
    let reason = over_limit_reason();
    let shown = b.shown();
    assert!(mentions(&shown, &reason), "the reason names the limit {MAX_RENDERS}: {shown:?}");
    let import = b.control("Import").expect("an Import button");
    assert!(!import.is_disabled(), "Import stays enabled");
}

// covers: PICK-8
#[test]
fn escape_on_the_surface_returns_to_the_source_libraries_page_and_starts_nothing() {
    let mut b = Bench::pair("escape", &["Alert", "Button", "Chip"], &["Button", "Slider"]);
    b.open_settings();
    b.click("Import components");
    b.wait_until("both cards are listed", |b| b.has("Slider") && b.has("Alert"));
    assert!(b.window_open(), "the surface is showing");
    b.click("Alert");
    assert!(b.checked("Alert"), "a component is ticked");

    let mark = b.mark();
    b.press(Key::Escape, Modifiers::NONE);
    assert!(!b.window_open(), "Escape closes the surface: {:?}", b.shown());
    assert!(b.flat().contains("Installed in this project"), "and returns to the Source libraries page: {:?}", b.shown());
    assert!(import_calls_since(&b, mark).is_empty(), "nothing is imported");
    assert_eq!(started_jobs(&b), 0, "no job but the readings was submitted");
}

// covers: PICK-9
#[test]
fn a_selection_over_the_render_limit_is_refused_on_import_naming_the_limit_and_a_small_one_goes_on() {
    let mut b = grid_open("over-limit", &[("Slim", 0, 0), ("Wide", 9, 4)]);
    b.click("Wide");
    let mark = b.mark();
    b.click("Import");
    assert_refused_for_the_limit(&mut b, mark);

    b.click("Wide");
    b.click("Slim");
    b.click("Import");
    let calls = import_calls_since(&b, mark);
    assert_eq!(calls.len(), 1, "a selection within the limit starts: {:?}", b.shown());
    assert_eq!(calls[0]["component"], json!("Slim"));
}

// covers: PICK-9
#[test]
fn the_limit_counts_the_renders_of_every_ticked_component_together() {
    let mut b = grid_open("sum", &[("Left", 4, 2), ("Right", 4, 2)]);
    b.click("Left");
    b.click("Right");
    let mark = b.mark();
    b.click("Import");
    assert_refused_for_the_limit(&mut b, mark);

    b.click("Right");
    b.click("Import");
    let calls = import_calls_since(&b, mark);
    assert_eq!(calls.len(), 1, "each alone is within the limit: {:?}", b.shown());
    assert_eq!(calls[0]["component"], json!("Left"));
}

// covers: PICK-9
#[test]
fn exactly_the_render_limit_is_allowed_and_one_more_state_is_not() {
    let mut b = grid_open("boundary", &[("Grid", 4, 4)]);
    b.click("Grid");
    let mark = b.mark();
    b.click("Import");
    assert_refused_for_the_limit(&mut b, mark);

    b.click("Hover");
    b.click("Import");
    let calls = import_calls_since(&b, mark);
    assert_eq!(calls.len(), 1, "4 styles x 4 sizes x 4 states x 2 themes is {MAX_RENDERS}: {:?}", b.shown());
    assert_eq!(calls[0]["component"], json!("Grid"));
}

// covers: PICK-10
#[test]
fn a_component_of_a_family_shows_under_one_card_named_after_the_family() {
    let sources = [
        json!({"package": HEROUI_STYLES, "version": FAMILY_VERSION, "license": "MIT", "removed": false}),
        json!({"package": HEROUI_REACT, "version": FAMILY_VERSION, "framework": "react", "license": "MIT", "removed": false}),
    ];
    let inspects = vec![
        (HEROUI_STYLES.to_string(), inspect_of(HEROUI_STYLES, FAMILY_VERSION, &[], &[])),
        (HEROUI_REACT.to_string(), inspect_of(HEROUI_REACT, FAMILY_VERSION, &["Alert", "Button"], &[])),
    ];
    let mut b = Bench::with("family-card", &sources, inspects);
    b.open(None);
    b.wait_until("the components are listed", |b| b.has("Alert") && b.has("Button"));
    assert!(b.window_open(), "the Choose your components surface is showing");

    let shown = b.shown();
    let cards = shown.iter().filter(|text| text.contains("heroui 3.2.7")).count();
    assert_eq!(cards, 1, "one card for the two packages, named after the family: {shown:?}");
    assert!(at(&shown, "heroui 3.2.7") < at(&shown, "Alert"), "the component is under the family card: {shown:?}");
    assert!(!mentions(&shown, "@heroui/"), "no package name is a card heading: {shown:?}");
    assert!(mentions(&shown, "0 of 2"), "the card counts the components of the family: {shown:?}");

    b.click("Alert");
    assert!(mentions(&b.shown(), "1 of 2"), "{:?}", b.shown());
}
