//! Opening or creating a library project from the Home screen, driven headless: the two action
//! cards, `open_project_at` on a temporary folder (a project opens its first design file, a folder
//! without `uigen.json` only says so, `create` starts a project in an empty folder with a first
//! component named in the UI language and never touches what exists), the folder picker, the
//! project's design file in the Recents, and the new words in every catalog.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use egui::accesskit::{Node, Role};
use figcraft_engine::Session;
use serde_json::json;

use crate::i18n::{self, Lang, Text};
use crate::layers_drag_tests::Ui;
use crate::{DialogFuture, FigApp, Services};

static NEXT: AtomicUsize = AtomicUsize::new(0);

const MISSING: &str = "No uigen.json in this folder. Use New library project to start one.";

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-project-open-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn design_bytes(page_name: &str) -> Vec<u8> {
    let mut doc = figcraft_doc::Document::new("lib");
    let page = doc.pages[0];
    doc.node_mut(page).unwrap().name = page_name.to_string();
    figcraft_format::save_bytes(&doc, None).unwrap()
}

fn write_project(dir: &Path, design_files: &[(&str, &str)]) {
    let names: Vec<&str> = design_files.iter().map(|(file, _)| *file).collect();
    let manifest = json!({"version": 1, "name": "mylib", "designFiles": names});
    std::fs::write(dir.join("uigen.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    for (file, page) in design_files {
        std::fs::write(dir.join(file), design_bytes(page)).unwrap();
    }
}

fn entries(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, at: &Path, out: &mut Vec<String>) {
        let Ok(read) = std::fs::read_dir(at) else { return };
        for entry in read.flatten() {
            let path = entry.path();
            out.push(path.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
            if path.is_dir() {
                walk(base, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn app_in(language: &str) -> Ui {
    figcraft_text::scan_system_fonts();
    let mut u = Ui::new(Session::empty());
    u.app.session.prefs.language = language.into();
    u.frame(vec![]);
    u.app.ui.home = true;
    u.app.ui.toast = None;
    u
}

fn toast(u: &Ui) -> Option<&str> {
    u.app.ui.toast.as_ref().map(|(message, _)| message.as_str())
}

fn file_name(path: Option<&str>) -> Option<String> {
    path.and_then(|p| Path::new(p).file_name()).map(|n| n.to_string_lossy().into_owned())
}

fn component_names(u: &mut Ui) -> Vec<String> {
    let info = u.app.session.execute("project.info", &json!({})).unwrap();
    info["components"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap().to_owned()).collect()
}

fn buttons(u: &mut Ui) -> Vec<Node> {
    u.ctx.enable_accesskit();
    u.frame(vec![]);
    u.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
}

// covers: L1-H1
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn the_home_screen_offers_open_project_and_new_library_project_as_buttons() {
    let mut u = app_in("en");
    let nodes = buttons(&mut u);
    for label in ["Open project", "New library project"] {
        let found = nodes.iter().filter(|n| n.role() == Role::Button && n.label() == Some(label)).count();
        assert_eq!(found, 1, "one button named {label:?}");
    }
}

// covers: L1-H2
#[test]
fn open_project_at_a_project_folder_opens_its_first_design_file_and_leaves_the_home_screen() {
    let dir = Dir::new("open");
    write_project(&dir.0, &[("a.figcraft", "A"), ("b.figcraft", "B")]);
    let before = entries(&dir.0);
    let mut u = app_in("en");
    assert!(u.app.open_project_at(dir.0.clone(), false));
    assert!(!u.app.ui.home, "the editor shows the project");
    assert_eq!(u.app.session.docs.len(), 2);
    assert_eq!(file_name(u.app.session.ds().unwrap().path.as_deref()).as_deref(), Some("a.figcraft"), "the first design file is in front");
    assert!(u.app.session.in_lib_project());
    assert_eq!(entries(&dir.0), before, "opening writes nothing");
}

// covers: L1-H2
#[test]
fn open_project_at_a_folder_without_uigen_json_toasts_and_writes_nothing() {
    let dir = Dir::new("missing");
    std::fs::write(dir.path("notes.txt"), "keep").unwrap();
    let before = entries(&dir.0);
    let mut u = app_in("en");
    assert!(!u.app.open_project_at(dir.0.clone(), false));
    assert_eq!(toast(&u), Some(MISSING));
    assert!(u.app.ui.home, "still on the home screen");
    assert!(u.app.session.docs.is_empty());
    assert_eq!(entries(&dir.0), before, "no file created");
    assert_eq!(std::fs::read_to_string(dir.path("notes.txt")).unwrap(), "keep");
}

struct PicksFolder(PathBuf);

impl Services for PicksFolder {
    fn pick_folder(&mut self, _title: &str) -> DialogFuture {
        Box::pin(std::future::ready(Some(self.0.clone())))
    }
}

// covers: L1-H2
#[test]
fn the_open_project_dialog_picks_a_folder_then_opens_it_as_a_project() {
    i18n::set("en", None);
    let dir = Dir::new("picker");
    write_project(&dir.0, &[("lib.figcraft", "Button")]);
    let mut app = FigApp::new(Session::empty(), Box::new(PicksFolder(dir.0.clone())));
    app.ui.home = true;
    app.open_project_dialog(false);
    assert!(app.session.docs.is_empty(), "opening waits for the picker result");
    app.poll_dialog(&egui::Context::default());
    assert_eq!(app.session.docs.len(), 1);
    assert!(app.session.in_lib_project());
    assert!(!app.ui.home);
}

// covers: L1-H3
#[test]
fn new_library_project_in_an_empty_folder_creates_the_project_with_component_1() {
    let parent = Dir::new("create-en");
    let dir = parent.path("my-lib");
    std::fs::create_dir_all(&dir).unwrap();
    let mut u = app_in("en");
    assert!(u.app.open_project_at(dir.clone(), true));
    assert!(!u.app.ui.home);
    let found = entries(&dir);
    for expected in ["uigen.json", "lib.figcraft"] {
        assert!(found.iter().any(|e| e == expected), "{expected} in {found:?}");
    }
    assert_eq!(component_names(&mut u), ["Component 1"]);
    assert!(u.app.session.in_lib_project());
}

// covers: L1-H3
#[test]
fn the_first_component_is_named_in_the_ui_language() {
    let parent = Dir::new("create-fr");
    let dir = parent.path("ma-lib");
    std::fs::create_dir_all(&dir).unwrap();
    let mut u = app_in("fr");
    assert!(u.app.open_project_at(dir.clone(), true));
    assert_eq!(component_names(&mut u), ["Composant 1"]);
}

// covers: L1-H3
#[test]
fn new_library_project_in_a_non_empty_folder_keeps_every_other_file() {
    let dir = Dir::new("create-keep");
    std::fs::write(dir.path("notes.txt"), "keep me").unwrap();
    std::fs::create_dir_all(dir.path("assets")).unwrap();
    std::fs::write(dir.path("assets/logo.svg"), "<svg/>").unwrap();
    let mut u = app_in("en");
    assert!(u.app.open_project_at(dir.0.clone(), true));
    assert_eq!(std::fs::read_to_string(dir.path("notes.txt")).unwrap(), "keep me");
    assert_eq!(std::fs::read_to_string(dir.path("assets/logo.svg")).unwrap(), "<svg/>");
    assert!(dir.path("uigen.json").is_file());
}

// covers: L1-H3
#[test]
fn new_library_project_on_an_existing_project_only_opens_it() {
    let dir = Dir::new("create-existing");
    write_project(&dir.0, &[("lib.figcraft", "Button")]);
    std::fs::write(dir.path("notes.txt"), "keep").unwrap();
    let (entries_before, manifest, design) =
        (entries(&dir.0), std::fs::read(dir.path("uigen.json")).unwrap(), std::fs::read(dir.path("lib.figcraft")).unwrap());
    let mut u = app_in("en");
    assert!(u.app.open_project_at(dir.0.clone(), true));
    assert_eq!(entries(&dir.0), entries_before, "nothing created");
    assert_eq!(std::fs::read(dir.path("uigen.json")).unwrap(), manifest, "the manifest is not rewritten");
    assert_eq!(std::fs::read(dir.path("lib.figcraft")).unwrap(), design, "the design file is not overwritten");
    assert_eq!(component_names(&mut u), ["Button"], "the existing page is kept, nothing is added");
    assert!(u.app.session.in_lib_project());
}

// covers: L1-H4
#[test]
fn a_programmatic_open_folder_never_opens_a_dialog() {
    let mut u = app_in("en");
    let ctx = u.ctx.clone();
    for command in ["project.openFolder", "project.newFolder"] {
        crate::control::handle(&mut u.app, &ctx, "engine.execute", &json!({"command": command})).unwrap();
        assert!(u.app.pending_dialog.is_none(), "{command}: the control chain drops the dialog request");
        assert!(u.app.session.ui_requests.is_empty());
    }
}

// covers: L1-H4
#[test]
fn a_user_open_folder_command_starts_the_folder_picker() {
    let mut u = app_in("en");
    assert!(u.app.pending_dialog.is_none());
    assert!(u.app.run("project.openFolder", json!({})).is_some());
    assert!(u.app.pending_dialog.is_some(), "the request opens the picker");
}

fn same_file(a: &str, b: &Path) -> bool {
    matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(a), Ok(b)) if a == b)
}

// covers: L1-H5
#[test]
fn an_opened_project_appears_in_the_recents_by_its_design_file() {
    let dir = Dir::new("recents-open");
    write_project(&dir.0, &[("lib.figcraft", "Button")]);
    let mut u = app_in("en");
    assert!(u.app.open_project_at(dir.0.clone(), false));
    let recents = &u.app.session.prefs.recent_files;
    assert!(recents.iter().any(|r| same_file(r, &dir.path("lib.figcraft"))), "{recents:?}");
}

// covers: L1-H5
#[test]
fn a_created_project_appears_in_the_recents_by_its_design_file() {
    let dir = Dir::new("recents-create");
    let mut u = app_in("en");
    assert!(u.app.open_project_at(dir.0.clone(), true));
    let recents = &u.app.session.prefs.recent_files;
    assert!(recents.iter().any(|r| same_file(r, &dir.path("lib.figcraft"))), "{recents:?}");
}

const KEYS: [&str; 7] = [
    "Open project",
    "A folder with uigen.json",
    "New library project",
    "Start a project in an empty folder",
    "Open project folder",
    MISSING,
    "Component {n}",
];

fn text_of<'a>(lang: &'a Lang, key: &str) -> Option<&'a str> {
    lang.ui.iter().find(|(k, _)| *k == key).map(|(_, t)| match t {
        Text::Plain(s) | Text::Plural { other: s, .. } => *s,
    })
}

fn has(lang: &Lang, key: &str) -> bool {
    text_of(lang, key).is_some_and(|s| !s.trim().is_empty())
}

// covers: L1-H6
#[test]
fn the_new_words_are_in_every_catalog_and_french_reads_composant() {
    let langs = i18n::languages();
    assert_eq!(langs.len(), 22);
    for lang in langs {
        for key in KEYS {
            let inherited = lang.parent != 0 && langs.get(lang.parent).is_some_and(|parent| has(parent, key));
            assert!(has(lang, key) || inherited, "{}: no non-empty {key:?} key in locales/{}.toml", lang.code, lang.code);
        }
    }
    let fr = langs.iter().find(|l| l.code == "fr").expect("French is baked in");
    assert_eq!(text_of(fr, "Component {n}"), Some("Composant {n}"));
    let en = langs.first().expect("English comes first");
    assert_eq!(text_of(en, "Component {n}"), Some("Component {n}"));
}
