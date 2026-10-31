//! File ▸ Project settings…: the window of a library project whose "Source libraries" section adds
//! an npm package, changes its version and removes it through the capture service, without ever
//! blocking a frame. Programmatic calls never open it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use egui::accesskit::{Node, Role};
use egui::{Event, Pos2, pos2};
use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureJob, JobId};
use figcraft_engine::catalog::MENU;
use serde_json::{Value, json};

use crate::dialogs::project_settings::{self, ProjectSettings, SourceRow};
use crate::i18n::{self, Lang, Text};
use crate::import_report_tests::shown_texts;
use crate::layers_drag_tests::Ui;

static NEXT: AtomicUsize = AtomicUsize::new(0);

const MUI: &str = "@mui/material";
const HEROUI_STYLES: &str = "@heroui/styles";
const HEROUI_REACT: &str = "@heroui/react";

type Outcome = Result<Value, CaptureError>;

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-sources-ui-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    fn manifest_sources(&self) -> Value {
        let manifest: Value = serde_json::from_slice(&std::fs::read(self.path("uigen.json")).unwrap()).unwrap();
        manifest["sources"].clone()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct Calls {
    submitted: Vec<CaptureJob>,
    ready: Vec<(JobId, Outcome)>,
}

struct FakeBackend {
    calls: Arc<Mutex<Calls>>,
}

impl CaptureBackend for FakeBackend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let mut calls = self.calls.lock().unwrap();
        calls.submitted.push(job);
        u64::try_from(calls.submitted.len()).unwrap()
    }

    fn poll(&mut self) -> Vec<(JobId, Outcome)> {
        std::mem::take(&mut self.calls.lock().unwrap().ready)
    }

    fn shutdown(&mut self) {}
}

fn mui_source(version: &str) -> Value {
    json!({"package": MUI, "version": version, "framework": "react", "license": "MIT", "removed": false})
}

fn write_project(dir: &Dir, sources: &[Value]) {
    let manifest = json!({"version": 1, "name": "mylib", "designFiles": ["lib.figcraft"], "sources": sources});
    std::fs::write(dir.path("uigen.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    let mut doc = figcraft_doc::Document::new("lib");
    let page = doc.pages[0];
    doc.node_mut(page).unwrap().name = "Button".to_string();
    std::fs::write(dir.path("lib.figcraft"), figcraft_format::save_bytes(&doc, None).unwrap()).unwrap();
}

struct Bench {
    ui: Ui,
    dir: Dir,
    calls: Arc<Mutex<Calls>>,
}

impl Bench {
    fn new(tag: &str, sources: &[Value]) -> Self {
        figcraft_text::scan_system_fonts();
        let dir = Dir::new(tag);
        write_project(&dir, sources);
        let mut ui = Ui::new(figcraft_engine::Session::empty());
        ui.app.session.prefs.language = "en".into();
        assert!(ui.app.open_project_at(dir.0.clone(), false));
        let calls = Arc::new(Mutex::new(Calls::default()));
        ui.app.session.set_capture_backend(Box::new(FakeBackend { calls: Arc::clone(&calls) }));
        ui.app.ui.toast = None;
        ui.app.ui.home = false;
        Self { ui, dir, calls }
    }

    fn open_window(&mut self) {
        assert!(self.ui.app.run("project.settings", json!({})).is_some());
        self.settle();
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.ui.frame(vec![]);
        }
    }

    fn state(&self) -> &ProjectSettings {
        self.ui.app.ui.project_settings.as_ref().expect("the project settings window is open")
    }

    fn row(&self, package: &str) -> Option<&SourceRow> {
        self.state().sources().iter().find(|r| r.package == package)
    }

    fn painted(&mut self) -> Vec<String> {
        self.ui.frame(vec![]);
        shown_texts(self.ui.frame_output(vec![]).shapes)
    }

    fn submitted(&self) -> Vec<CaptureJob> {
        self.calls.lock().unwrap().submitted.clone()
    }

    fn submitted_shapes(&self) -> Vec<(String, String, Option<String>)> {
        self.submitted()
            .into_iter()
            .flat_map(|job| match job {
                CaptureJob::Install { package, version, .. } => vec![("install".to_string(), package, Some(version))],
                CaptureJob::InstallMany { packages, .. } => {
                    packages.into_iter().map(|spec| ("install".to_string(), spec.name, Some(spec.version))).collect()
                }
                CaptureJob::Uninstall { package, .. } => vec![("uninstall".to_string(), package, None)],
                other => panic!("unexpected job {other:?}"),
            })
            .collect()
    }

    fn install_many_jobs(&self) -> Vec<Vec<(String, String)>> {
        self.submitted()
            .into_iter()
            .map(|job| match job {
                CaptureJob::InstallMany { packages, .. } => packages.into_iter().map(|spec| (spec.name, spec.version)).collect(),
                other => panic!("expected an InstallMany job, got {other:?}"),
            })
            .collect()
    }

    fn finish_last(&mut self, outcome: Outcome) {
        let job = {
            let mut calls = self.calls.lock().unwrap();
            let job = u64::try_from(calls.submitted.len()).unwrap();
            calls.ready.push((job, outcome));
            job
        };
        assert!(job > 0, "no job was submitted");
        self.settle();
    }

    fn toast(&self) -> Option<String> {
        self.ui.app.ui.toast.as_ref().map(|t| t.0.clone())
    }

    fn nodes(&mut self) -> Vec<Node> {
        self.ui.ctx.enable_accesskit();
        self.ui.frame(vec![]);
        self.ui.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
    }

    fn text_inputs(&mut self) -> Vec<Node> {
        self.nodes().into_iter().filter(|n| n.role() == Role::TextInput).collect()
    }

    fn field_value(&mut self) -> String {
        let fields = self.text_inputs();
        fields.first().and_then(|n| n.value()).unwrap_or_default().to_string()
    }

    fn type_line(&mut self, line: &str) {
        let fields = self.text_inputs();
        let field = fields.first().expect("the text field is on screen");
        let b = field.bounds().expect("the text field has bounds");
        let at = pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
        self.ui.button(at, true);
        self.ui.button(at, false);
        self.settle();
        self.ui.frame(vec![Event::Text(line.to_string())]);
        self.settle();
    }

    fn has_label(&mut self, label: &str) -> bool {
        self.nodes().iter().any(|n| n.label().is_some_and(|l| l.trim() == label))
    }

    fn centre_of(&mut self, label: &str) -> Pos2 {
        let all = self.nodes();
        let node = all.iter().find(|n| n.label().is_some_and(|l| l.trim() == label)).unwrap_or_else(|| panic!("no {label:?} on screen"));
        let b = node.bounds().unwrap_or_else(|| panic!("{label:?} has no bounds"));
        pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
    }

    fn click(&mut self, label: &str) {
        let at = self.centre_of(label);
        self.ui.button(at, true);
        self.ui.button(at, false);
        self.settle();
    }
}

fn installed(package: &str, version: &str) -> Outcome {
    Ok(json!({"package": package, "version": version, "framework": "react", "license": "MIT"}))
}

fn installed_all(list: &[(&str, &str)]) -> Outcome {
    let packages: Vec<Value> =
        list.iter().map(|(package, version)| json!({"package": package, "version": version, "framework": "react", "license": "MIT"})).collect();
    Ok(json!({"packages": packages}))
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(name, version)| (name.to_string(), version.to_string())).collect()
}

fn mentions(shown: &[String], needle: &str) -> bool {
    shown.iter().any(|t| t.contains(needle))
}

fn mentions_ci(shown: &[String], needle: &str) -> bool {
    let needle = needle.to_lowercase();
    shown.iter().any(|t| t.to_lowercase().contains(&needle))
}

#[test]
fn the_file_menu_offers_project_settings_which_opens_the_window_for_a_user_only() {
    // covers: E2-7.1
    let item = MENU.iter().find(|m| m.id == "project.settings").expect("a menu item runs project.settings");
    assert_eq!(item.menu, "File");
    assert!(item.label.starts_with("Project settings"), "{:?}", item.label);
    assert!(crate::menus::available("project.settings"));

    let mut b = Bench::new("open", &[mui_source("7.3.11")]);
    assert!(b.ui.app.ui.project_settings.is_none());
    b.ui.app.session.ui_requests.clear();
    b.ui.app.session.execute("project.settings", &json!({})).unwrap();
    assert!(b.ui.app.session.ui_requests.iter().any(|r| r.kind == "dialog.projectSettings"), "{:?}", b.ui.app.session.ui_requests);
    b.ui.app.session.ui_requests.clear();

    let ctx = b.ui.ctx.clone();
    crate::control::handle(&mut b.ui.app, &ctx, "engine.execute", &json!({"command": "project.settings"})).unwrap();
    b.settle();
    assert!(b.ui.app.ui.project_settings.is_none(), "the control channel opened the window");
    assert!(b.ui.app.session.ui_requests.is_empty());

    b.open_window();
    let shown = b.painted();
    for text in ["Source libraries", "Run", "Package", "Version", "Framework", "Close"] {
        assert!(mentions(&shown, text), "{text:?} is on screen: {shown:?}");
    }
    assert!(mentions(&shown, MUI) && mentions(&shown, "7.3.11"), "the listed source is on screen: {shown:?}");
}

#[test]
fn adding_a_package_shows_an_installing_row_without_blocking_then_its_version_framework_and_license() {
    // covers: E2-7.1
    let mut b = Bench::new("add", &[]);
    b.open_window();
    assert!(b.state().sources().is_empty());

    project_settings::add_packages(&mut b.ui.app, MUI);
    for _ in 0..30 {
        b.ui.frame(vec![]);
    }
    assert_eq!(b.submitted_shapes(), vec![("install".to_string(), MUI.to_string(), Some("latest".to_string()))]);
    let row = b.row(MUI).expect("the row appears at once");
    assert_eq!(row.status, "installing");
    let shown = b.painted();
    assert!(mentions(&shown, "Installing…") && mentions(&shown, MUI), "the installing row is on screen while the service works: {shown:?}");
    assert_eq!(b.dir.manifest_sources(), json!([]), "uigen.json waits for the success");

    b.finish_last(installed_all(&[(MUI, "7.3.11")]));
    let row = b.row(MUI).expect("the row stays");
    assert_eq!((row.status.as_str(), row.version.as_str()), ("ready", "7.3.11"));
    let shown = b.painted();
    assert!(mentions(&shown, "7.3.11"), "{shown:?}");
    assert!(mentions_ci(&shown, "react"), "{shown:?}");
    assert!(mentions(&shown, "MIT"), "{shown:?}");
    assert!(!mentions(&shown, "Installing…"), "{shown:?}");
    assert_eq!(b.dir.manifest_sources()[0]["version"], "7.3.11");
    assert!(b.toast().is_some_and(|t| !t.is_empty()), "the end of the job is toasted");
}

#[test]
fn a_failed_install_says_what_failed_and_why_and_adds_nothing() {
    // covers: E2-7.2
    let cause = "npm error 404 Not Found - GET https://registry.invalid/@mui%2fmaterial";
    let mut b = Bench::new("fail", &[]);
    b.open_window();
    project_settings::add_packages(&mut b.ui.app, MUI);
    b.settle();
    assert_eq!(b.row(MUI).map(|r| r.status.as_str()), Some("installing"));

    b.finish_last(Err(CaptureError { kind: CaptureErrorKind::Install, message: cause.into() }));
    let shown = b.painted();
    assert!(
        shown.iter().any(|t| t.contains(cause) && t.to_lowercase().contains("install")),
        "a message names the installation and the cause: {shown:?}"
    );
    assert!(b.row(MUI).is_none(), "no row is kept: {:?}", b.state().sources());
    assert_eq!(b.dir.manifest_sources(), json!([]));
}

#[test]
fn an_invalid_package_name_is_refused_in_the_field_with_the_reason_and_submits_nothing() {
    // covers: E2-7.4
    let mut b = Bench::new("invalid", &[]);
    b.open_window();
    assert!(b.state().add_error().is_none());

    project_settings::add_packages(&mut b.ui.app, "Not Valid!");
    b.settle();
    let reason = b.state().add_error().expect("the field shows why the name is refused").to_string();
    assert!(reason.contains("lowercase registry name"), "{reason:?}");
    let shown = b.painted();
    assert!(mentions(&shown, &reason), "the reason is on screen: {shown:?}");
    assert!(b.submitted().is_empty(), "no work was submitted: {:?}", b.submitted());
    assert!(b.state().sources().is_empty());
    let status = b.ui.app.session.execute("capture.status", &json!({})).unwrap();
    assert_eq!(status["jobs"], json!([]));
    assert_eq!(b.dir.manifest_sources(), json!([]));
}

#[test]
fn remove_asks_confirmation_then_the_row_stays_greyed_as_a_removed_source() {
    // covers: E2-7.3
    let mut b = Bench::new("remove", &[mui_source("7.3.11")]);
    b.open_window();
    assert_eq!(b.row(MUI).map(|r| r.status.as_str()), Some("ready"));

    b.click("Remove");
    assert_eq!(b.state().pending_removal(), Some(MUI), "the click only asks");
    assert!(b.submitted().is_empty(), "nothing is uninstalled before the confirmation");
    b.click("Cancel");
    assert_eq!(b.state().pending_removal(), None);
    assert!(b.submitted().is_empty());
    assert_eq!(b.row(MUI).map(|r| r.status.as_str()), Some("ready"));

    b.click("Remove");
    b.click("Confirm removal");
    assert_eq!(b.submitted_shapes(), vec![("uninstall".to_string(), MUI.to_string(), None)]);
    b.finish_last(Ok(json!({"package": MUI})));

    let row = b.row(MUI).expect("the removed source stays in the list");
    assert_eq!((row.status.as_str(), row.version.as_str()), ("removed", "7.3.11"));
    let shown = b.painted();
    assert!(mentions(&shown, "Removed source") && mentions(&shown, MUI), "{shown:?}");
    assert!(!b.has_label("Remove"), "a removed source offers no removal");
    assert!(!b.has_label("Change version"), "a removed source offers no version change");
    let sources = b.dir.manifest_sources();
    assert_eq!((sources[0]["package"].as_str(), sources[0]["removed"].as_bool()), (Some(MUI), Some(true)));
    assert!(b.toast().is_some_and(|t| !t.is_empty()), "the end of the job is toasted");
}

#[test]
fn changing_the_version_shows_the_new_one_only_after_the_install_succeeded() {
    // covers: E2-7.3
    let mut b = Bench::new("version", &[mui_source("7.3.11")]);
    b.open_window();
    assert!(b.has_label("Change version"), "a ready source offers the change");

    project_settings::change_version(&mut b.ui.app, MUI, "7.4.0");
    b.settle();
    assert_eq!(b.submitted_shapes(), vec![("install".to_string(), MUI.to_string(), Some("7.4.0".to_string()))]);
    assert_eq!(b.row(MUI).map(|r| r.version.as_str()), Some("7.3.11"), "the old version stays while the service works");
    let shown = b.painted();
    assert!(mentions(&shown, "7.3.11") && !mentions(&shown, "7.4.0"), "{shown:?}");

    b.finish_last(installed(MUI, "7.4.0"));
    assert_eq!(b.row(MUI).map(|r| r.version.as_str()), Some("7.4.0"));
    let shown = b.painted();
    assert!(mentions(&shown, "7.4.0") && !mentions(&shown, "7.3.11"), "{shown:?}");
    assert_eq!(b.dir.manifest_sources()[0]["version"], "7.4.0");
}

#[test]
fn a_failed_version_change_keeps_the_old_version() {
    // covers: E2-7.3
    let mut b = Bench::new("version-fail", &[mui_source("7.3.11")]);
    b.open_window();
    project_settings::change_version(&mut b.ui.app, MUI, "7.4.0");
    b.settle();
    b.finish_last(Err(CaptureError { kind: CaptureErrorKind::Install, message: "npm error ETARGET no matching version".into() }));
    assert_eq!(b.row(MUI).map(|r| r.version.as_str()), Some("7.3.11"));
    let shown = b.painted();
    assert!(mentions(&shown, "7.3.11") && !mentions(&shown, "7.4.0"), "{shown:?}");
    assert_eq!(b.dir.manifest_sources()[0]["version"], "7.3.11");
}

// covers: INST-6
#[test]
fn the_window_has_one_text_field_and_an_install_button_and_no_version_field() {
    let mut b = Bench::new("install-form", &[mui_source("7.3.11")]);
    b.open_window();

    let fields = b.text_inputs();
    let labels: Vec<Option<&str>> = fields.iter().map(Node::label).collect();
    assert_eq!(fields.len(), 1, "{labels:?}");
    assert_ne!(labels[0].map(str::trim), Some("Version"));
    assert!(b.has_label("Run"), "the Run button is on screen");
    let shown = b.painted();
    assert!(!mentions(&shown, "Add an npm package"), "{shown:?}");
}

// covers: INST-6
#[test]
fn clicking_install_runs_add_sources_with_the_typed_line_and_both_packages_show_as_installing() {
    let mut b = Bench::new("install-click", &[]);
    b.open_window();

    b.type_line("npm i @heroui/styles @heroui/react");
    b.click("Run");

    assert_eq!(b.install_many_jobs(), vec![pairs(&[(HEROUI_STYLES, "latest"), (HEROUI_REACT, "latest")])]);
    let status = b.ui.app.session.execute("capture.status", &json!({})).unwrap();
    assert_eq!(status["jobs"].as_array().map(Vec::len), Some(1), "{status}");
    assert_eq!(status["jobs"][0]["command"], json!("project.addSources"), "{status}");
    for package in [HEROUI_STYLES, HEROUI_REACT] {
        assert_eq!(b.row(package).map(|r| r.status.as_str()), Some("installing"), "{package}");
    }
    let shown = b.painted();
    assert!(mentions(&shown, HEROUI_STYLES) && mentions(&shown, HEROUI_REACT) && mentions(&shown, "Installing…"), "{shown:?}");
    assert_eq!(b.dir.manifest_sources(), json!([]), "uigen.json waits for the success");

    b.finish_last(installed_all(&[(HEROUI_STYLES, "2.0.1"), (HEROUI_REACT, "2.4.0")]));
    assert_eq!(b.row(HEROUI_STYLES).map(|r| (r.status.as_str(), r.version.as_str())), Some(("ready", "2.0.1")));
    assert_eq!(b.row(HEROUI_REACT).map(|r| (r.status.as_str(), r.version.as_str())), Some(("ready", "2.4.0")));
    let sources = b.dir.manifest_sources();
    assert_eq!(sources.as_array().map(Vec::len), Some(2), "{sources}");
}

// covers: INST-6
#[test]
fn a_refused_line_shows_the_reason_keeps_the_typed_text_and_starts_no_job() {
    let mut b = Bench::new("install-refused", &[]);
    b.open_window();

    b.type_line("npm uninstall a");
    b.click("Run");

    let reason = b.state().add_error().expect("the window shows why the line is refused").to_string();
    assert!(reason.contains("uninstall"), "{reason:?}");
    let shown = b.painted();
    assert!(mentions(&shown, &reason), "the reason is on screen: {shown:?}");
    assert_eq!(b.field_value(), "npm uninstall a");
    assert!(b.submitted().is_empty(), "no work was submitted: {:?}", b.submitted());
    assert!(b.state().sources().is_empty());
    let status = b.ui.app.session.execute("capture.status", &json!({})).unwrap();
    assert_eq!(status["jobs"], json!([]));
    assert_eq!(b.dir.manifest_sources(), json!([]));
}

// covers: INST-6
#[test]
fn add_packages_refuses_a_shell_line_or_an_empty_one_without_starting_a_job() {
    let mut b = Bench::new("install-direct", &[]);
    b.open_window();

    for (line, token) in [("a && b", "&&"), ("", "")] {
        project_settings::add_packages(&mut b.ui.app, line);
        b.settle();
        let reason = b.state().add_error().unwrap_or_else(|| panic!("{line:?} is refused with a reason")).to_string();
        assert!(!reason.trim().is_empty() && reason.contains(token), "{line:?}: {reason:?}");
    }
    assert!(b.submitted().is_empty(), "{:?}", b.submitted());
    assert!(b.state().sources().is_empty());
}

fn keys_in(src: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for call in ["tr(\"", "trf(\"", "trn(\"", "tr_noop(\""] {
        for (at, _) in src.match_indices(call) {
            if src[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let mut key = String::new();
            let mut chars = src[at + call.len()..].chars();
            while let Some(c) = chars.next() {
                match c {
                    '"' => {
                        found.insert(key);
                        break;
                    }
                    '\\' => key.extend(chars.next().map(|n| if n == 'n' { '\n' } else { n })),
                    c => key.push(c),
                }
            }
        }
    }
    found
}

const FIXED_KEYS: [&str; 12] = [
    "Source libraries",
    "Run",
    "Package",
    "Version",
    "Framework",
    "Remove",
    "Change version",
    "Removed source",
    "Installing…",
    "Confirm removal",
    "Cancel",
    "Close",
];

const FRENCH: [(&str, &str); 9] = [
    ("Source libraries", "Bibliothèques sources"),
    ("Run", "Lancer"),
    ("Package", "Paquet"),
    ("Framework", "Framework"),
    ("Remove", "Retirer"),
    ("Change version", "Changer de version"),
    ("Removed source", "Source retirée"),
    ("Installing…", "Installation…"),
    ("Close", "Fermer"),
];

fn text_of<'a>(lang: &'a Lang, key: &str) -> Option<&'a str> {
    lang.ui.iter().find(|(k, _)| *k == key).map(|(_, t)| match t {
        Text::Plain(s) | Text::Plural { other: s, .. } => *s,
    })
}

#[test]
fn every_language_translates_the_window_strings_and_the_command_label() {
    // covers: E2-7.5
    let languages = i18n::languages();
    assert_eq!(languages.len(), 22);
    let dialogs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/dialogs");
    let mut sources = vec![dialogs.join("project_settings.rs")];
    if let Ok(entries) = std::fs::read_dir(dialogs.join("project_settings")) {
        sources.extend(entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "rs")));
    }
    let mut keys = BTreeSet::new();
    for path in &sources {
        keys.extend(keys_in(&std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))));
    }
    for key in FIXED_KEYS {
        assert!(keys.contains(key), "the window uses the key {key:?}: {keys:?}");
    }

    let mut missing = Vec::new();
    for lang in languages.iter().filter(|l| l.code != "en") {
        let own: Vec<&Lang> = [Some(lang), languages.get(lang.parent)].into_iter().flatten().filter(|l| l.code != "en").collect();
        for key in &keys {
            if !own.iter().any(|l| l.ui.iter().any(|(k, _)| k == key)) {
                missing.push(format!("{}: {key:?}", lang.code));
            }
        }
        if !own.iter().any(|l| l.commands.iter().any(|(k, _)| *k == "project.settings")) {
            missing.push(format!("{}: [commands] project.settings", lang.code));
        }
    }
    assert!(missing.is_empty(), "untranslated:\n{}", missing.join("\n"));

    let fr = languages.iter().find(|l| l.code == "fr").expect("French is baked in");
    for (key, expected) in FRENCH {
        assert_eq!(text_of(fr, key), Some(expected), "fr: {key:?}");
    }
}
