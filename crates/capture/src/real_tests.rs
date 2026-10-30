//! The real capture service against the real npm registry, Node and Chromium of this machine.
//! Every test is `#[ignore]`d because it downloads: run them by hand with
//! `cargo test -p figcraft-capture -- --ignored --test-threads=1`.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureJob, CaptureOutcome};
use serde_json::{Value, json};

use crate::service_tests::TempDir;
use crate::{CaptureService, locate};

const PACKAGE: &str = "@mui/material";
const VERSION: &str = "7.3.11";
const ABSENT_PACKAGE: &str = "@uigen-test/absent-404";
const JOB_LIMIT: Duration = Duration::from_secs(900);
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

fn real_service() -> CaptureService {
    let config = locate().expect("node and the sidecar script must be found (UIGEN_NODE, UIGEN_SIDECAR_DIR)");
    CaptureService::start(config, Box::new(|| {}))
}

fn root_of(dir: &TempDir) -> String {
    dir.path().display().to_string()
}

fn run(service: &mut CaptureService, job: CaptureJob) -> CaptureOutcome {
    let id = service.submit(job);
    let deadline = Instant::now() + JOB_LIMIT;
    loop {
        if let Some((answered, outcome)) = service.poll().into_iter().next() {
            assert_eq!(answered, id);
            return outcome;
        }
        assert!(Instant::now() < deadline, "no result within {JOB_LIMIT:?}");
        thread::sleep(Duration::from_millis(50));
    }
}

fn install(service: &mut CaptureService, root: &str, package: &str, version: &str) -> CaptureOutcome {
    run(service, CaptureJob::Install { root: root.to_string(), package: package.to_string(), version: version.to_string() })
}

fn render(service: &mut CaptureService, root: &str, component: &str, props: Value) -> CaptureOutcome {
    run(
        service,
        CaptureJob::Render {
            root: root.to_string(),
            package: PACKAGE.to_string(),
            component: component.to_string(),
            props,
            sample_text: None,
            state: "default".to_string(),
            theme: "light".to_string(),
        },
    )
}

fn installed_package_dir(root: &Path) -> std::path::PathBuf {
    root.join(".uigen").join("sources").join("node_modules").join("@mui").join("material")
}

fn assert_readable(error: &CaptureError) {
    assert!(error.message.trim().len() > 10, "the message is not readable: {:?}", error.message);
}

// covers: E2-8.1
#[test]
#[ignore = "downloads @mui/material from npm; run by hand"]
fn real_service_installs_inspects_renders_and_uninstalls_a_mui_package() {
    let dir = TempDir::new("real-mui");
    let root = root_of(&dir);
    let mut service = real_service();

    let installed = install(&mut service, &root, PACKAGE, VERSION).unwrap();
    assert_eq!(installed["package"], PACKAGE);
    assert_eq!(installed["version"], VERSION);
    assert_eq!(installed["framework"], "react");
    assert!(installed_package_dir(dir.path()).is_dir(), "the package is not in .uigen/sources/node_modules");

    let inspected = run(&mut service, CaptureJob::Inspect { root: root.clone(), package: PACKAGE.to_string() }).unwrap();
    let components = inspected["components"].as_array().expect("components is a list");
    assert!(components.iter().any(|component| component["name"] == "Button"), "Button is not listed");

    let rendered = render(&mut service, &root, "Button", json!({"variant": "contained"})).unwrap();
    let snapshot = figcraft_dom::parse_snapshot(&rendered["snapshot"].to_string()).expect("the snapshot is accepted by figcraft-dom");
    assert_eq!(snapshot.version, figcraft_dom::SUPPORTED_VERSION);
    let png = std::fs::read(dir.path().join(rendered["png"].as_str().expect("png is a relative path"))).unwrap();
    assert!(png.starts_with(&PNG_SIGNATURE), "the capture is not a PNG file");
    assert!(png.len() > PNG_SIGNATURE.len(), "the PNG is empty");

    let removed = run(&mut service, CaptureJob::Uninstall { root: root.clone(), package: PACKAGE.to_string() }).unwrap();
    assert_eq!(removed["package"], PACKAGE);
    assert!(!installed_package_dir(dir.path()).exists(), "the package is still in .uigen/sources/node_modules");

    let asked = Instant::now();
    service.shutdown();
    assert!(asked.elapsed() < crate::Timeouts::default().shutdown, "the service did not exit by itself after shutdown");
}

// covers: E2-8.2
#[test]
#[ignore = "asks the npm registry for a package that does not exist; run by hand"]
fn real_service_reports_a_missing_package_and_a_missing_component_readably() {
    let dir = TempDir::new("real-errors");
    let root = root_of(&dir);
    let mut service = real_service();

    let missing = install(&mut service, &root, ABSENT_PACKAGE, "1.0.0").unwrap_err();
    assert_eq!(missing.kind, CaptureErrorKind::Install);
    assert_readable(&missing);

    install(&mut service, &root, PACKAGE, VERSION).unwrap();
    let unknown = render(&mut service, &root, "Nope", json!({})).unwrap_err();
    assert_eq!(unknown.kind, CaptureErrorKind::ComponentNotFound);
    assert_readable(&unknown);

    service.shutdown();
}
