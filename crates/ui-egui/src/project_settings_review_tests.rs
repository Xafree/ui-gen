//! The project settings window reads the capture jobs without running a command, and shows only
//! the jobs of its own project.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureJob, JobId};
use serde_json::{Value, json};

use crate::dialogs::project_settings::SourceRow;
use crate::layers_drag_tests::Ui;

static NEXT: AtomicUsize = AtomicUsize::new(0);

type Outcome = Result<Value, CaptureError>;

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-settings-review-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = json!({"version": 1, "name": "mylib", "designFiles": ["lib.figcraft"]});
        std::fs::write(dir.join("uigen.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
        let mut doc = figcraft_doc::Document::new("lib");
        let page = doc.pages[0];
        doc.node_mut(page).unwrap().name = "Button".to_string();
        std::fs::write(dir.join("lib.figcraft"), figcraft_format::save_bytes(&doc, None).unwrap()).unwrap();
        Self(dir)
    }

    fn root(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct IdleBackend {
    submitted: Arc<Mutex<usize>>,
}

impl CaptureBackend for IdleBackend {
    fn submit(&mut self, _job: CaptureJob) -> JobId {
        let mut count = self.submitted.lock().unwrap();
        *count += 1;
        u64::try_from(*count).unwrap()
    }

    fn poll(&mut self) -> Vec<(JobId, Outcome)> {
        Vec::new()
    }

    fn shutdown(&mut self) {}
}

struct Bench {
    ui: Ui,
    dir: Dir,
}

impl Bench {
    fn new(tag: &str) -> Self {
        figcraft_text::scan_system_fonts();
        let dir = Dir::new(tag);
        let mut ui = Ui::new(figcraft_engine::Session::empty());
        ui.app.session.prefs.language = "en".into();
        assert!(ui.app.open_project_at(dir.0.clone(), false));
        ui.app.session.set_capture_backend(Box::new(IdleBackend { submitted: Arc::default() }));
        ui.app.ui.home = false;
        Self { ui, dir }
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

    fn rows(&self) -> Vec<SourceRow> {
        self.ui.app.ui.project_settings.as_ref().expect("the project settings window is open").sources().to_vec()
    }
}

fn install(root: &str, package: &str) -> CaptureJob {
    CaptureJob::Install { root: root.to_string(), package: package.to_string(), version: "latest".to_string() }
}

// covers: E2-7.1
#[test]
fn an_open_window_leaves_the_journal_and_the_last_command_alone_frame_after_frame() {
    let mut bench = Bench::new("frames");
    bench.open_window();
    let journal = bench.ui.app.session.journal.len();
    let last = bench.ui.app.session.last_command.clone();

    for _ in 0..20 {
        bench.ui.frame(vec![]);
    }

    assert_eq!(bench.ui.app.session.journal.len(), journal);
    assert_eq!(bench.ui.app.session.last_command, last);
}

// covers: E2-7.1
#[test]
fn the_window_shows_the_jobs_of_its_project_and_not_those_of_another() {
    let mut bench = Bench::new("scope");
    let other = Dir::new("scope-other");
    bench.ui.app.session.submit_capture("project.addSource", "left-pad", install(&other.root(), "left-pad")).unwrap();
    bench.ui.app.session.submit_capture("project.addSource", "lit", install(&bench.dir.root(), "lit")).unwrap();

    bench.open_window();

    let packages: Vec<String> = bench.rows().into_iter().map(|row| row.package).collect();
    assert_eq!(packages, vec!["lit".to_string()]);
}
