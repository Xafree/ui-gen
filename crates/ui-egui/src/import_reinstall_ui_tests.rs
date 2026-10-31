use figcraft_engine::capture::{CaptureError, CaptureErrorKind, CaptureJob};

use crate::import_window_ui_tests::{Bench, OTHER, OTHER_VERSION, Outcome, PACKAGE, VERSION, failure, inspect_of, mentions, source};

const MESSAGE: &str = "These libraries are not installed in this project.";

fn not_installed(package: &str) -> Outcome {
    Err(CaptureError { kind: CaptureErrorKind::NotInstalled, message: format!("{package} is not installed in this project") })
}

fn both_missing(tag: &str) -> Bench {
    let sources = [source(PACKAGE, VERSION, false), source(OTHER, OTHER_VERSION, false)];
    let mut b = Bench::with(tag, &sources, vec![(PACKAGE.to_string(), not_installed(PACKAGE)), (OTHER.to_string(), not_installed(OTHER))]);
    b.open(None);
    b.wait_until("the message is drawn", |b| b.has("Reinstall"));
    b
}

#[test]
fn missing_libraries_show_one_message_one_reinstall_and_no_try_again() {
    let mut b = both_missing("reinstall-one");
    let shown = b.shown();
    assert_eq!(shown.iter().filter(|line| line.contains(MESSAGE)).count(), 1, "{shown:?}");
    assert_eq!(b.controls("Reinstall").len(), 1);
    assert!(!b.has("Try again"), "{shown:?}");
    assert!(!mentions(&shown, "is not installed in this project"), "{shown:?}");
}

#[test]
fn reinstall_runs_one_job_shows_its_loading_state_then_reads_the_components_again() {
    let mut b = both_missing("reinstall-ok");
    b.set_inspect(PACKAGE, inspect_of(PACKAGE, VERSION, &["Button"], &[]));
    b.set_inspect(OTHER, inspect_of(OTHER, VERSION, &["Chip"], &[]));
    b.hold();
    b.click("Reinstall");
    b.settle();
    assert!(b.shows("Reinstalling…"), "{:?}", b.shown());
    let installs: Vec<CaptureJob> = b.submitted().into_iter().filter(|job| matches!(job, CaptureJob::InstallMany { .. })).collect();
    assert_eq!(installs.len(), 1, "{installs:?}");

    b.release_all();
    b.wait_until("the components are listed", |b| b.has("Button") && b.has("Chip"));
    assert!(!b.shows(MESSAGE), "{:?}", b.shown());
    assert!(!b.has("Reinstall"));
}

#[test]
fn a_failed_reinstall_says_why_and_keeps_the_button() {
    let mut b = both_missing("reinstall-failed");
    b.shared.lock().unwrap().failing_install = Some(CaptureError { kind: CaptureErrorKind::Install, message: "no network".to_string() });
    b.click("Reinstall");
    b.wait_until("the failure is drawn", |b| b.shows("Couldn't reinstall the libraries: no network"));
    assert!(b.has("Reinstall"));
    assert!(b.shows(MESSAGE));
}

#[test]
fn other_read_errors_keep_try_again() {
    let mut b = Bench::with("reinstall-other", &[source(PACKAGE, VERSION, false)], vec![(PACKAGE.to_string(), Err(failure("no type declarations")))]);
    b.open(None);
    b.wait_until("the error is drawn", |b| b.has("Try again"));
    assert!(!b.has("Reinstall"));
}
