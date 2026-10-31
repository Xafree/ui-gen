//! Source families on the Source libraries page: what a version change and a removal send, and the
//! state of the page when the project in front changes. The bench is the one of `import_window_ui_tests`.

use std::path::Path;

use figcraft_engine::capture::CaptureJob;
use serde_json::{Value, json};

use crate::dialogs::project_settings::{add_packages, change_version};
use crate::import_window_ui_tests::{Bench, Dir, PACKAGE, VERSION, source};

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
    assert_eq!(sent, [(SAME.to_string(), "6.0.0".to_string()), (HEAD.to_string(), "6.0.0".to_string())], "the dependency at another version is left alone");
}
