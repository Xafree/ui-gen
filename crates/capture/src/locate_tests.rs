//! `locate` reads `UIGEN_NODE`, `UIGEN_SIDECAR_DIR`, `PATH` and the executable's own folder. The
//! test process never changes its own environment (`set_var` is unsafe, and tests run in parallel):
//! each case relaunches the test executable on `locate_probe_child_entry` with the environment it
//! needs, and that probe writes what `locate` returned to a JSON file.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use crate::locate;
use crate::service_tests::TempDir;

const PROBE_OUT: &str = "FIGCRAFT_LOCATE_PROBE_OUT";
const DEFAULT_MAX_RESPONSE_BYTES: u64 = 32 * 1024 * 1024;

#[test]
fn locate_probe_child_entry() {
    let Some(out) = std::env::var_os(PROBE_OUT) else {
        return;
    };
    let report = match locate() {
        Ok(config) => json!({
            "ok": true,
            "node": config.node.display().to_string(),
            "script": config.script.display().to_string(),
            "hello_secs": config.timeouts.hello.as_secs(),
            "install_secs": config.timeouts.install.as_secs(),
            "inspect_secs": config.timeouts.inspect.as_secs(),
            "render_secs": config.timeouts.render.as_secs(),
            "shutdown_secs": config.timeouts.shutdown.as_secs(),
            "max_response_bytes": config.max_response_bytes,
        }),
        Err(error) => json!({"ok": false, "kind": format!("{:?}", error.kind), "message": error.message}),
    };
    fs::write(out, report.to_string()).unwrap();
}

#[derive(Default)]
struct Environment<'a> {
    node: Option<&'a Path>,
    sidecar_dir: Option<&'a Path>,
}

fn run_probe(exe: &Path, scratch: &TempDir, path_dir: &Path, environment: &Environment) -> Value {
    let out = scratch.path().join("probe-report.json");
    let _ = fs::remove_file(&out);
    let mut command = Command::new(exe);
    command
        .args(["--exact", "locate_tests::locate_probe_child_entry", "--test-threads=1"])
        .env(PROBE_OUT, &out)
        .env("PATH", path_dir)
        .env_remove("UIGEN_NODE")
        .env_remove("UIGEN_SIDECAR_DIR");
    if let Some(node) = environment.node {
        command.env("UIGEN_NODE", node);
    }
    if let Some(dir) = environment.sidecar_dir {
        command.env("UIGEN_SIDECAR_DIR", dir);
    }
    let output = command.output().unwrap();
    assert!(output.status.success(), "probe failed: {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap()
}

fn current_exe() -> PathBuf {
    std::env::current_exe().unwrap()
}

fn executable_in(dir: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let file = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    fs::write(&file, b"").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
    }
    file
}

fn script_in(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let file = dir.join("service.mjs");
    fs::write(&file, b"").unwrap();
    file
}

fn text<'a>(report: &'a Value, key: &str) -> &'a str {
    report[key].as_str().unwrap_or_else(|| panic!("report has no `{key}`: {report}"))
}

fn assert_same_file(reported: &str, expected: &Path) {
    let reported = fs::canonicalize(reported).unwrap_or_else(|error| panic!("`{reported}` does not exist: {error}"));
    assert_eq!(reported, fs::canonicalize(expected).unwrap());
}

// covers: E2-5.6
#[test]
fn locate_uses_uigen_node_and_uigen_sidecar_dir_and_the_default_limits() {
    let scratch = TempDir::new("locate-env");
    let node = executable_in(&scratch.path().join("runtime"), "my-node");
    let script = script_in(&scratch.path().join("service-dir"));
    let empty_path = scratch.path().join("empty-path");
    fs::create_dir_all(&empty_path).unwrap();

    let report = run_probe(&current_exe(), &scratch, &empty_path, &Environment { node: Some(&node), sidecar_dir: script.parent() });

    assert_eq!(report["ok"], true, "report: {report}");
    assert_same_file(text(&report, "node"), &node);
    assert_same_file(text(&report, "script"), &script);
    assert_eq!(report["hello_secs"], 15);
    assert_eq!(report["install_secs"], 600);
    assert_eq!(report["inspect_secs"], 60);
    assert_eq!(report["render_secs"], 30);
    assert_eq!(report["shutdown_secs"], 5);
    assert_eq!(report["max_response_bytes"], DEFAULT_MAX_RESPONSE_BYTES);
}

// covers: E2-5.6
#[test]
fn locate_looks_for_node_in_path_when_uigen_node_is_not_set() {
    let scratch = TempDir::new("locate-path");
    let path_dir = scratch.path().join("on-path");
    let node = executable_in(&path_dir, "node");
    let script = script_in(&scratch.path().join("service-dir"));

    let report = run_probe(&current_exe(), &scratch, &path_dir, &Environment { node: None, sidecar_dir: script.parent() });

    assert_eq!(report["ok"], true, "report: {report}");
    assert_same_file(text(&report, "node"), &node);
}

// covers: E2-5.6
#[test]
fn locate_looks_for_the_script_beside_the_executable_unless_uigen_sidecar_dir_is_set() {
    let scratch = TempDir::new("locate-beside");
    let bin_dir = scratch.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let copy = bin_dir.join(format!("capture-tests{}", std::env::consts::EXE_SUFFIX));
    fs::copy(current_exe(), &copy).unwrap();
    let beside = script_in(&bin_dir.join("sidecar"));
    let elsewhere = script_in(&scratch.path().join("elsewhere"));
    let node = executable_in(&scratch.path().join("runtime"), "my-node");
    let empty_path = scratch.path().join("empty-path");
    fs::create_dir_all(&empty_path).unwrap();

    let found = run_probe(&copy, &scratch, &empty_path, &Environment { node: Some(&node), sidecar_dir: None });
    let overridden = run_probe(&copy, &scratch, &empty_path, &Environment { node: Some(&node), sidecar_dir: elsewhere.parent() });

    assert_eq!(found["ok"], true, "report: {found}");
    assert_same_file(text(&found, "script"), &beside);
    assert_eq!(overridden["ok"], true, "report: {overridden}");
    assert_same_file(text(&overridden, "script"), &elsewhere);
}

// covers: E2-5.6
#[cfg(debug_assertions)]
#[test]
fn locate_falls_back_to_the_repository_sidecar_in_a_debug_build() {
    let scratch = TempDir::new("locate-repo");
    let node = executable_in(&scratch.path().join("runtime"), "my-node");
    let empty_path = scratch.path().join("empty-path");
    fs::create_dir_all(&empty_path).unwrap();

    let report = run_probe(&current_exe(), &scratch, &empty_path, &Environment { node: Some(&node), sidecar_dir: None });

    assert_eq!(report["ok"], true, "report: {report}");
    assert_same_file(text(&report, "script"), &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sidecar/service.mjs"));
}

// covers: E2-5.6
#[test]
fn locate_without_a_node_is_a_start_error_listing_the_places_searched() {
    let scratch = TempDir::new("locate-no-node");
    let path_dir = scratch.path().join("searched-for-node");
    fs::create_dir_all(&path_dir).unwrap();
    let script = script_in(&scratch.path().join("service-dir"));

    let report = run_probe(&current_exe(), &scratch, &path_dir, &Environment { node: None, sidecar_dir: script.parent() });

    assert_eq!(report["ok"], false, "report: {report}");
    assert_eq!(report["kind"], "Start");
    let message = text(&report, "message");
    assert!(message.contains(&path_dir.display().to_string()), "message: {message}");
    assert!(message.contains("UIGEN_NODE"), "message: {message}");
}
