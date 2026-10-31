//! Source families on the Source libraries page: what a version change and a removal send, and the
//! state of the page when the project in front changes. The bench is the one of `import_window_ui_tests`.

use std::path::Path;

use figcraft_engine::capture::CaptureJob;
use serde_json::{Value, json};

use crate::dialogs::project_settings::{add_packages, change_version};
use crate::import_window_ui_tests::{Bench, Dir, PACKAGE};

const HEAD: &str = "@acme/ui";
const SAME: &str = "@acme/styles";
const ENGINE: &str = "acme-engine";

fn row(package: &str, version: &str, framework: Option<&str>, removed: bool) -> Value {
    json!({"package": package, "version": version, "framework": framework.unwrap_or("unknown"), "license": "MIT", "removed": removed})
}

fn depends(b: &Bench, package: &str, on: &[&str]) {
    let root = b.ui.app.session.project().expect("a project").root.clone();
    let folder = Path::new(&root).join(".uigen/sources/node_modules").join(package);
    std::fs::create_dir_all(&folder).unwrap();
    let dependencies: serde_json::Map<String, Value> = on.iter().map(|name| ((*name).to_string(), json!("*"))).collect();
    std::fs::write(folder.join("package.json"), json!({"dependencies": dependencies}).to_string()).unwrap();
}

fn last_job(b: &Bench) -> CaptureJob {
    b.shared.lock().unwrap().submitted.last().cloned().expect("a job was submitted")
}

fn jobs(b: &Bench) -> Vec<CaptureJob> {
    b.shared.lock().unwrap().submitted.clone()
}

#[test]
fn a_family_version_goes_only_to_the_members_that_share_the_version_of_the_head() {
    let sources = [row(HEAD, "3.0.0", Some("react"), false), row(SAME, "3.0.0", None, false), row(ENGINE, "18.3.1", None, false)];
    let mut b = Bench::with("family-version-share", &sources, Vec::new());
    depends(&b, HEAD, &[SAME, ENGINE]);
    b.open_settings();
    change_version(&mut b.ui.app, HEAD, "6.0.0");
    let CaptureJob::InstallMany { packages, .. } = last_job(&b) else { panic!("one job for the family: {:?}", jobs(&b)) };
    let mut sent: Vec<(String, String)> = packages.into_iter().map(|spec| (spec.name, spec.version)).collect();
    sent.sort();
    assert_eq!(
        sent,
        [(SAME.to_string(), "6.0.0".to_string()), (HEAD.to_string(), "6.0.0".to_string())],
        "the dependency at another version is left alone"
    );
}

#[test]
fn a_package_another_head_depends_on_stays_and_the_confirmation_names_what_goes_and_what_stays() {
    let other = "@other/kit";
    let shared = "shared-core";
    let sources = [
        row(HEAD, "3.0.0", Some("react"), false),
        row(other, "1.0.0", Some("react"), false),
        row(shared, "9.0.0", None, false),
        row(SAME, "3.0.0", None, false),
    ];
    let mut b = Bench::with("family-remove-shared", &sources, Vec::new());
    depends(&b, HEAD, &[shared, SAME]);
    depends(&b, other, &[shared]);
    b.open_settings();
    b.click_nth("Remove", 0);
    assert!(b.shows(&format!("Removes {HEAD}, {SAME}")), "{:?}", b.shown());
    assert!(b.shows(&format!("Keeps {shared}: another library needs them")), "{:?}", b.shown());
    b.click("Confirm removal");
    let gone: Vec<String> = jobs(&b)
        .into_iter()
        .filter_map(|job| match job {
            CaptureJob::Uninstall { package, .. } => Some(package),
            _ => None,
        })
        .collect();
    assert!(!gone.contains(&shared.to_string()), "{gone:?}");
    assert!(gone.contains(&HEAD.to_string()) && gone.contains(&SAME.to_string()), "{gone:?}");
}

#[test]
fn a_version_change_on_a_family_left_with_one_active_member_goes_to_that_member() {
    let sources = [row(HEAD, "3.0.0", Some("react"), true), row(SAME, "3.0.0", None, false)];
    let mut b = Bench::with("family-single", &sources, Vec::new());
    b.open_settings();
    change_version(&mut b.ui.app, HEAD, "4.0.0");
    let CaptureJob::Install { package, version, .. } = last_job(&b) else { panic!("{:?}", jobs(&b)) };
    assert_eq!((package.as_str(), version.as_str()), (SAME, "4.0.0"));
}

fn second_project(tag: &str) -> Dir {
    let dir = Dir::new(tag);
    let manifest = json!({"version": 1, "name": "second", "designFiles": ["lib.figcraft"], "sources": []});
    std::fs::write(dir.0.join("uigen.json"), manifest.to_string()).unwrap();
    let doc = figcraft_doc::Document::new("lib");
    std::fs::write(dir.0.join("lib.figcraft"), figcraft_format::save_bytes(&doc, None).unwrap()).unwrap();
    dir
}

#[test]
fn the_state_of_the_page_does_not_follow_another_project_to_the_front() {
    let mut b = Bench::nominal("settings-root", &["Alert"]);
    b.open_settings();
    add_packages(&mut b.ui.app, "npm uninstall a");
    let error = |b: &Bench| b.ui.app.ui.project_settings.as_ref().and_then(|state| state.add_error().map(str::to_string));
    assert!(error(&b).is_some(), "the refusal shows");
    let dir = second_project("settings-root-second");
    assert!(b.ui.app.open_project_at(dir.0.clone(), false), "the second project opens");
    b.settle();
    assert_eq!(error(&b), None, "the refusal stayed with its project");
}

#[test]
fn a_package_with_no_dependency_is_read_once_until_a_job_ends() {
    let dir = Dir::new("dependencies-cache");
    let root = dir.0.to_string_lossy().into_owned();
    assert!(crate::source_family::dependencies_of(&root, "late", "1.0.0").is_empty());
    let folder = dir.0.join(".uigen/sources/node_modules/late");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("package.json"), json!({"dependencies": {"now": "1.0.0"}}).to_string()).unwrap();
    assert!(crate::source_family::dependencies_of(&root, "late", "1.0.0").is_empty(), "the empty answer is kept");
    crate::source_family::forget_dependencies();
    assert_eq!(crate::source_family::dependencies_of(&root, "late", "1.0.0"), ["now"]);
}

#[test]
fn a_finished_install_forgets_what_was_read() {
    let mut b = Bench::with("dependencies-job", &[row(HEAD, "3.0.0", Some("react"), false)], Vec::new());
    let root = b.ui.app.session.project().expect("a project").root.clone();
    assert!(crate::source_family::dependencies_of(&root, HEAD, "3.0.0").is_empty());
    depends(&b, HEAD, &[SAME]);
    add_packages(&mut b.ui.app, PACKAGE);
    b.settle();
    assert_eq!(crate::source_family::dependencies_of(&root, HEAD, "3.0.0"), [SAME]);
}

#[test]
fn an_install_line_goes_through_the_way_every_command_of_the_ui_goes() {
    let mut b = Bench::nominal("add-through-run", &["Alert"]);
    b.open_settings();
    crate::FigApp::clear_run_log();
    add_packages(&mut b.ui.app, "other-lib");
    assert_eq!(crate::FigApp::runs_of("project.addSources").len(), 1, "it is in the run log");
}

#[test]
fn a_package_already_there_is_said_in_words_the_catalog_translates() {
    let mut b = Bench::nominal("refusal-source", &["Alert"]);
    b.open_settings();
    add_packages(&mut b.ui.app, PACKAGE);
    let error = b.ui.app.ui.project_settings.as_ref().and_then(|state| state.add_error().map(str::to_string));
    assert_eq!(error.as_deref(), Some("synthetic-ui is already a source of this project"));
}

#[test]
fn a_change_already_running_is_said_in_words_the_catalog_translates() {
    let mut b = Bench::nominal("refusal-running", &["Alert"]);
    b.open_settings();
    b.hold();
    change_version(&mut b.ui.app, PACKAGE, "2.0.0");
    change_version(&mut b.ui.app, PACKAGE, "3.0.0");
    assert_eq!(b.ui.app.ui.last_error.as_deref(), Some("A change to synthetic-ui is already in progress"));
}
