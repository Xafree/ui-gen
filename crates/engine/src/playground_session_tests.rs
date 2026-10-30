//! The engine opens, refreshes and drives the playground through the capture port (E7-4.1 to
//! E7-4.8): the commands export the applied theme into the project's staging folder, hand `playground.*`
//! jobs to the capture service, keep the open page's status, relay its images and the user's input,
//! and leave the jobs of `import.*` and `project.*` alone. The service is a scripted fake backend.

use std::sync::Arc;

use serde_json::{Value, json};

use crate::capture::{CaptureErrorKind, CaptureEvent, CaptureJob};
use crate::file_access::{Roots, confined, same_file};
use crate::playground::{FrameFormat, Status, Template};
use crate::playground_support::{APPLIED_TOKEN, Bench, FRAME, Library, OPEN, OTHER_TOKEN, failure, file_names, image};
use crate::{EngineError, all_commands};

const JPEG_123: &str = "AQID";
const JPEG_456: &str = "BAUG";
const JPEG_999: &str = "CQkJ";
const KEYS: [&str; 14] = [
    "Enter",
    "Tab",
    "Escape",
    "Backspace",
    "Delete",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Space",
];

fn is_bad(result: &crate::Result<Value>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

fn message_of(result: crate::Result<Value>) -> String {
    match result {
        Err(error) => error.to_string(),
        Ok(value) => panic!("expected a refusal, got {value}"),
    }
}

fn events_of<'a>(events: &'a [CaptureEvent], command: &str) -> Vec<&'a CaptureEvent> {
    events.iter().filter(|event| event.command == command).collect()
}

fn refused_open(bench: &mut Bench, params: &Value, needle: &str) {
    let before = bench.document_text();
    let message = message_of(bench.run("playground.open", params));
    assert!(message.contains(needle), "{params}: {message}");
    assert!(is_bad(&bench.run("playground.open", params)), "{params} must be a bad-parameters refusal");
    assert!(bench.calls.submitted().is_empty(), "{params}: a job was submitted");
    assert!(bench.nothing_staged(), "{params}: something was written");
    assert_eq!(bench.document_text(), before, "{params}");
    assert!(matches!(bench.session.playground_status(), Status::Closed), "{params}");
}

fn window(width: u64, height: u64, scale: f64) -> Value {
    json!({"template": "components", "width": width, "height": height, "scale": scale})
}

fn assert_staged_package(bench: &Bench) {
    let lib = bench.staged("lib");
    assert!(lib.join("index.ts").is_file());
    let components: Vec<String> = file_names(&lib.join("components")).into_iter().filter(|name| name.ends_with(".ts")).collect();
    assert_eq!(components.len(), 2, "{components:?}");
    let tokens = std::fs::read_to_string(lib.join("tokens/tokens.css")).unwrap();
    assert!(tokens.contains(APPLIED_TOKEN), "the applied theme is in tokens.css");
    assert!(!tokens.contains(OTHER_TOKEN), "a collection that is not the applied theme is in tokens.css");
    let template = std::fs::read_to_string(bench.staged("template.html")).unwrap();
    assert_eq!(template, bench.composed(Template::Dashboard).html);
}

fn assert_open_job(bench: &Bench) {
    let jobs = bench.calls.submitted();
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    let CaptureJob::Playground { root, method, params } = &jobs[0] else { panic!("not a playground job: {:?}", jobs[0]) };
    assert_eq!(method, OPEN);
    assert!(same_file(root, bench.root()), "{root}");
    assert_eq!(params["tags"].as_array().map(Vec::len), Some(2), "{params}");
    assert_eq!(params["theme"], json!("light"), "{params}");
    for key in ["width", "height", "scale"] {
        assert!(params[key].is_number(), "{key} in {params}");
    }
}

// covers: E7-4.1
#[test]
fn open_exports_only_the_applied_theme_composes_the_template_and_becomes_ready() {
    let mut bench = Bench::new("open", Library::Full);
    let before = bench.document_text();

    let opened = bench.run("playground.open", &json!({"template": "dashboard"})).unwrap();

    assert!(opened["job"].is_u64(), "{opened}");
    assert_eq!(bench.document_text(), before, "opening touches the document");
    assert!(matches!(bench.session.playground_status(), Status::Opening));
    assert_staged_package(&bench);
    assert_open_job(&bench);

    let events = bench.pump();
    assert!(matches!(bench.session.playground_status(), Status::Ready));
    let opens = events_of(&events, OPEN);
    assert_eq!(opens.len(), 1, "{events:?}");
    assert!(opens[0].outcome.is_ok(), "{:?}", opens[0].outcome);
    assert_eq!(opens[0].job, opened["job"].as_u64().unwrap());
    assert_eq!(opens[0].package, "");
    let state = bench.state();
    assert_eq!(state["status"], json!("ready"), "{state}");
    assert_eq!(state["template"], json!("dashboard"), "{state}");
    assert_eq!(state["stale"], json!(false), "{state}");
}

// covers: E7-4.2
#[test]
fn open_refuses_a_project_that_cannot_be_shown_and_submits_and_writes_nothing() {
    let mut outside = Bench::standalone("outside");
    refused_open(&mut outside, &json!({}), "project");

    let mut no_variables = Bench::new("no-variables", Library::NoVariables);
    refused_open(&mut no_variables, &json!({}), "variable");

    let mut no_components = Bench::new("no-components", Library::NoComponents);
    refused_open(&mut no_components, &json!({}), "component");

    let mut not_ready = Bench::new("not-ready", Library::DuplicateNames);
    refused_open(&mut not_ready, &json!({}), "Button");

    let mut no_backend = Bench::without_backend("no-backend", Library::Full);
    refused_open(&mut no_backend, &json!({}), "capture service not available");
}

// covers: E7-4.2
#[test]
fn open_refuses_an_unknown_template_and_a_window_out_of_bounds() {
    let mut bench = Bench::new("bounds", Library::Full);
    refused_open(&mut bench, &json!({"template": "nope"}), "nope");
    let refused = [
        window(319, 600, 1.0),
        window(3841, 600, 1.0),
        window(800, 239, 1.0),
        window(800, 2161, 1.0),
        window(800, 600, 0.9),
        window(800, 600, 2.1),
        json!({"width": "wide"}),
        json!({"scale": -1}),
    ];
    for params in refused {
        refused_open(&mut bench, &params, "");
    }
}

// covers: E7-4.2
#[test]
fn open_refuses_a_staging_folder_that_cannot_be_written() {
    let mut bench = Bench::new("unwritable", Library::Full);
    let elsewhere = crate::playground_support::Dir::new("elsewhere");
    let roots = Arc::new(Roots::new(&[bench.root()], &[elsewhere.root()]).unwrap());

    let result = confined(Some(&roots), || bench.run("playground.open", &json!({})));

    assert!(is_bad(&result), "{result:?}");
    assert!(bench.calls.submitted().is_empty());
    assert!(bench.nothing_staged());
    assert!(matches!(bench.session.playground_status(), Status::Closed));
}

// covers: E7-4.2
#[test]
fn open_accepts_the_window_bounds() {
    for (width, height, scale) in [(320_u64, 240_u64, 1.0_f64), (3840, 2160, 2.0)] {
        let mut bench = Bench::new("window", Library::Full);
        bench.run("playground.open", &window(width, height, scale)).unwrap();
        let params = bench.calls.playground(OPEN).remove(0);
        assert_eq!(params["width"].as_f64(), Some(width as f64), "{params}");
        assert_eq!(params["height"].as_f64(), Some(height as f64), "{params}");
        assert_eq!(params["scale"].as_f64(), Some(scale), "{params}");
    }
}

// covers: E7-4.2
#[test]
fn playground_commands_survive_junk_parameters_closed_or_open() {
    let junk = [
        json!({}),
        json!(null),
        json!([1, 2, 3]),
        json!({"template": {"id": 1}, "width": 1e308, "height": "NaN", "scale": -1}),
        json!({"name": ["x"], "events": "move", "template": "\u{0}".repeat(5000)}),
        json!({"events": [{"type": "key", "key": null}, 5, {"type": "move", "x": 1e308, "y": -1e308}]}),
        json!({"events": vec![json!({"type": "move", "x": 1, "y": 1}); 5000], "name": "\u{0}"}),
    ];
    let ids: Vec<&str> = all_commands().iter().map(|spec| spec.id).filter(|id| id.starts_with("playground.")).collect();
    for expected in ["playground.open", "playground.refresh", "playground.close", "playground.template", "playground.state", "playground.input"] {
        assert!(ids.contains(&expected), "{expected} is not a command: {ids:?}");
    }
    for mut bench in [Bench::new("junk-closed", Library::Full), Bench::ready("junk-open")] {
        for id in &ids {
            for params in &junk {
                if let Err(EngineError::Internal { cmd, msg }) = bench.run(id, params) {
                    panic!("{cmd} panicked on {params}: {msg}");
                }
            }
        }
    }
}

// covers: E7-4.3
#[test]
fn a_failed_open_leaves_failed_with_the_cause_and_the_document_untouched() {
    let mut bench = Bench::new("failed", Library::Full);
    bench.calls.script(|script| script.open = Err(failure(CaptureErrorKind::Render, "components/ui-button.ts: unexpected token")));
    let before = bench.document_text();

    bench.run("playground.open", &json!({})).unwrap();
    let events = bench.pump();

    assert!(matches!(bench.session.playground_status(), Status::Failed(message) if message.contains("unexpected token")));
    let opens = events_of(&events, OPEN);
    assert_eq!(opens.len(), 1, "{events:?}");
    assert!(matches!(&opens[0].outcome, Err(error) if error.kind == CaptureErrorKind::Render && error.message.contains("unexpected token")));
    assert_eq!(bench.document_text(), before);
    let state = bench.state();
    assert_eq!(state["status"], json!("failed"), "{state}");
    assert!(state["error"].as_str().is_some_and(|error| error.contains("unexpected token")), "{state}");
}

// covers: E7-4.3
#[test]
fn refresh_exports_again_and_reopens() {
    let mut bench = Bench::new("refresh", Library::Full);
    bench.calls.script(|script| script.open = Err(failure(CaptureErrorKind::Render, "the page crashed")));
    bench.run("playground.open", &json!({"template": "mail"})).unwrap();
    bench.pump();
    assert!(matches!(bench.session.playground_status(), Status::Failed(_)));
    bench.calls.script(|script| script.open = Ok(json!({"defined": [], "missing": [], "errors": [], "shape": true})));
    std::fs::remove_file(bench.staged("lib/index.ts")).unwrap();

    let refreshed = bench.run("playground.refresh", &json!({})).unwrap();
    let events = bench.pump();

    assert!(refreshed["job"].is_u64(), "{refreshed}");
    assert!(bench.staged("lib/index.ts").is_file(), "the package was not exported again");
    assert_eq!(bench.calls.playground(OPEN).len(), 2);
    let refreshes = events_of(&events, "playground.refresh");
    assert_eq!(refreshes.len(), 1, "{events:?}");
    assert!(refreshes[0].outcome.is_ok());
    assert!(matches!(bench.session.playground_status(), Status::Ready));
    assert_eq!(bench.state()["template"], json!("mail"));
}

// covers: E7-4.3
#[test]
fn a_timeout_or_an_exited_service_fails_the_playground_without_a_panic() {
    for kind in [CaptureErrorKind::Timeout, CaptureErrorKind::ServiceExited] {
        let mut opening = Bench::new("open-down", Library::Full);
        opening.calls.script(|script| script.open = Err(failure(kind, "the capture service stopped")));
        opening.run("playground.open", &json!({})).unwrap();
        opening.pump();
        assert!(matches!(opening.session.playground_status(), Status::Failed(message) if message.contains("stopped")), "{kind:?} on open");

        let mut ready = Bench::ready("frame-down");
        ready.calls.script(|script| script.frames.push_back(Err(failure(kind, "the capture service stopped"))));
        ready.cycle();
        ready.pump();
        assert!(matches!(ready.session.playground_status(), Status::Failed(_)), "{kind:?} on frame");
        assert_eq!(ready.state()["status"], json!("failed"));
    }
}

// covers: E7-4.4
#[test]
fn template_recomposes_the_page_without_opening_it_again() {
    let mut bench = Bench::ready("template");
    let opens = bench.calls.playground(OPEN).len();

    let changed = bench.run("playground.template", &json!({"name": "mail"})).unwrap();

    let composed = bench.composed(Template::Mail);
    assert_eq!(std::fs::read_to_string(bench.staged("template.html")).unwrap(), composed.html);
    let missing: Vec<String> = changed["missing"].as_array().unwrap().iter().map(|name| name.as_str().unwrap().to_string()).collect();
    assert_eq!(missing, composed.missing);
    assert_eq!(bench.calls.playground("playground.setTemplate").len(), 1);
    assert_eq!(bench.calls.playground(OPEN).len(), opens, "the page was opened again");
    bench.pump();
    assert!(matches!(bench.session.playground_status(), Status::Ready));
    assert_eq!(bench.state()["template"], json!("mail"));
}

// covers: E7-4.4
#[test]
fn template_refuses_an_unknown_name_and_changes_nothing() {
    let mut bench = Bench::ready("template-unknown");
    let page = std::fs::read_to_string(bench.staged("template.html")).unwrap();

    let result = bench.run("playground.template", &json!({"name": "nope"}));

    assert!(is_bad(&result), "{result:?}");
    assert_eq!(std::fs::read_to_string(bench.staged("template.html")).unwrap(), page);
    assert!(bench.calls.playground("playground.setTemplate").is_empty());
}

// covers: E7-4.5
#[test]
fn poll_asks_for_one_image_at_a_time_and_only_when_ready() {
    let mut closed = Bench::new("poll-closed", Library::Full);
    assert!(!closed.session.playground_poll());
    assert!(closed.calls.playground(FRAME).is_empty());

    let mut opening = Bench::new("poll-opening", Library::Full);
    opening.run("playground.open", &json!({})).unwrap();
    opening.session.playground_poll();
    assert!(opening.calls.playground(FRAME).is_empty(), "an image was asked for before the page was ready");

    let mut failed = Bench::new("poll-failed", Library::Full);
    failed.calls.script(|script| script.open = Err(failure(CaptureErrorKind::Render, "broken")));
    failed.run("playground.open", &json!({})).unwrap();
    failed.pump();
    failed.session.playground_poll();
    assert!(failed.calls.playground(FRAME).is_empty());

    let mut bench = Bench::ready("poll");
    bench.calls.script(|script| script.frames.push_back(image(1, "jpeg", 64, 48, JPEG_123)));
    let asked = bench.calls.playground(FRAME).len();
    bench.session.playground_poll();
    bench.session.playground_poll();
    let requests = bench.calls.playground(FRAME);
    assert_eq!(requests.len(), asked + 1, "two polls with a request in flight asked twice");
    assert_eq!(requests[asked]["after"], json!(0));

    bench.session.pump_capture();
    bench.session.playground_poll();
    let requests = bench.calls.playground(FRAME);
    assert_eq!(requests.len(), asked + 2, "no request after the answer came back");
    assert_eq!(requests[asked + 1]["after"], json!(1));
}

// covers: E7-4.5
#[test]
fn an_image_that_is_not_newer_or_not_valid_is_ignored_and_the_status_stays() {
    let too_large = "A".repeat(11_500_000);
    let mut bench = Bench::ready("images");
    bench.calls.script(|script| {
        script.frames.extend([
            image(5, "jpeg", 64, 48, JPEG_123),
            image(5, "jpeg", 64, 48, JPEG_999),
            image(3, "jpeg", 64, 48, JPEG_999),
            image(6, "jpeg", 64, 48, &too_large),
            image(7, "jpeg", 0, 48, JPEG_999),
            image(8, "jpeg", 64, 0, JPEG_999),
            image(9, "jpeg", 5_000_000_000, 48, JPEG_999),
            image(10, "jpeg", 64, 48, "!!! not base64 !!!"),
            image(11, "png", 64, 48, JPEG_456),
        ]);
    });

    bench.cycle();
    let first = bench.session.playground_take_frame().expect("the first valid image");
    assert_eq!((first.seq, first.width, first.height, first.data.as_slice()), (5, 64, 48, [1_u8, 2, 3].as_slice()));
    assert!(matches!(first.format, FrameFormat::Jpeg));

    for ignored in 2..=8 {
        bench.cycle();
        assert!(bench.session.playground_take_frame().is_none(), "image number {ignored} was taken");
        assert!(matches!(bench.session.playground_status(), Status::Ready), "image number {ignored} changed the status");
    }

    bench.cycle();
    let last = bench.session.playground_take_frame().expect("a valid image after the ignored ones");
    assert_eq!((last.seq, last.data.as_slice()), (11, [4_u8, 5, 6].as_slice()));
    assert!(matches!(last.format, FrameFormat::Png));
}

// covers: E7-4.5
#[test]
fn the_latest_valid_image_is_taken_once() {
    let mut bench = Bench::ready("take-once");
    bench.calls.script(|script| script.frames.extend([image(1, "jpeg", 64, 48, JPEG_123), image(2, "jpeg", 64, 48, JPEG_456)]));
    bench.cycle();
    bench.cycle();

    let frame = bench.session.playground_take_frame().expect("an image");

    assert_eq!((frame.seq, frame.data.as_slice()), (2, [4_u8, 5, 6].as_slice()), "the older image came out");
    assert!(bench.session.playground_take_frame().is_none(), "an image was taken twice");
    bench.cycle();
    assert!(bench.session.playground_take_frame().is_none(), "an unchanged page gave an image");
}

// covers: E7-4.6
#[test]
fn input_relays_valid_events_unchanged() {
    let mut bench = Bench::ready("input");
    let events = json!([
        {"type": "move", "x": 10.5, "y": 20.5},
        {"type": "down", "x": 10.5, "y": 20.5, "button": "left"},
        {"type": "up", "x": 10.5, "y": 20.5, "button": "right"},
        {"type": "wheel", "x": 1.5, "y": 2.5, "dx": 0.5, "dy": -120.5},
        {"type": "key", "key": "Enter", "mods": ["shift"]},
        {"type": "text", "text": "héllo"}
    ]);

    bench.run("playground.input", &json!({"events": events})).unwrap();

    let sent = bench.calls.playground("playground.input");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["events"], events);

    let every_key: Vec<Value> = KEYS.iter().map(|key| json!({"type": "key", "key": key})).collect();
    bench.run("playground.input", &json!({"events": every_key})).unwrap();
    let most: Vec<Value> = (0..64).map(|i| json!({"type": "move", "x": f64::from(i) + 0.5, "y": 1.5})).collect();
    bench.run("playground.input", &json!({"events": most})).unwrap();
    assert_eq!(bench.calls.playground("playground.input").len(), 3);
}

// covers: E7-4.6
#[test]
fn one_invalid_event_refuses_the_whole_batch() {
    let mut bench = Bench::ready("input-invalid");
    let valid = json!({"type": "move", "x": 1.5, "y": 2.5});
    let invalid = [
        json!({"type": "teleport", "x": 1.5, "y": 2.5}),
        json!({"type": "move", "y": 2.5}),
        json!({"type": "move", "x": "1.5", "y": 2.5}),
        json!({"type": "move", "x": 1.5, "y": null}),
        json!({"type": "down", "x": 1.5, "y": 2.5, "button": "back"}),
        json!({"type": "down", "x": 1.5, "y": 2.5}),
        json!({"type": "key", "key": "F13"}),
        json!({"type": "key", "key": "a"}),
        json!(5),
    ];
    for event in invalid {
        let result = bench.run("playground.input", &json!({"events": [valid, event, valid]}));
        assert!(is_bad(&result), "{event}: {result:?}");
    }
    let too_many: Vec<Value> = (0..65).map(|_| valid.clone()).collect();
    for events in [json!([]), json!(too_many), json!("move"), json!(null)] {
        let result = bench.run("playground.input", &json!({"events": events}));
        assert!(is_bad(&result), "{result:?}");
    }
    assert!(bench.calls.playground("playground.input").is_empty(), "a batch with an invalid event was submitted");
}

// covers: E7-4.6
#[test]
fn input_is_refused_when_the_playground_is_closed() {
    let events = json!({"events": [{"type": "move", "x": 1.5, "y": 2.5}]});
    let mut never_opened = Bench::new("input-closed", Library::Full);
    assert!(is_bad(&never_opened.run("playground.input", &events)));
    assert!(never_opened.calls.playground("playground.input").is_empty());

    let mut closed_again = Bench::ready("input-closed-again");
    closed_again.run("playground.close", &json!({})).unwrap();
    assert!(is_bad(&closed_again.run("playground.input", &events)));
    assert!(closed_again.calls.playground("playground.input").is_empty());
}

// covers: E7-4.7
#[test]
fn playground_jobs_are_not_listed_and_do_not_use_up_the_limit_of_finished_jobs() {
    let mut bench = Bench::ready("listing");
    let root = bench.root();
    let probe = bench.session.submit_capture("capture.probe", "pkg", CaptureJob::Inspect { root, package: "pkg".to_string() }).unwrap();
    for _ in 0..3 {
        bench.cycle();
    }

    let jobs = bench.session.capture_jobs();
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    assert_eq!((jobs[0].job, jobs[0].command.as_str()), (probe, "capture.probe"));
    let status = bench.run("capture.status", &json!({})).unwrap();
    assert_eq!(status["jobs"].as_array().map(Vec::len), Some(1), "{status}");

    bench.calls.release();
    bench.pump();
    for _ in 0..120 {
        bench.cycle();
    }
    let jobs = bench.session.capture_jobs();
    assert_eq!(jobs.len(), 1, "playground jobs pushed the finished job out of the list: {jobs:?}");
    assert_eq!(jobs[0].job, probe);
    assert!(bench.calls.playground(FRAME).len() > 100, "the test did not run enough image requests");
}

fn job_of(kind: usize, root: String) -> CaptureJob {
    let package = "pkg".to_string();
    match kind {
        0 => CaptureJob::Install { root, package, version: "1.0.0".to_string() },
        1 => CaptureJob::Inspect { root, package },
        _ => CaptureJob::Render {
            root,
            package,
            component: "Button".to_string(),
            props: json!({}),
            sample_text: None,
            state: "default".to_string(),
            theme: "light".to_string(),
        },
    }
}

// covers: E7-4.7
#[test]
fn busy_is_true_while_an_install_inspect_or_render_job_precedes() {
    for kind in 0..3 {
        let mut bench = Bench::ready("busy");
        assert_eq!(bench.state()["busy"], json!(false));
        let job = job_of(kind, bench.root());
        bench.session.submit_capture("capture.probe", "pkg", job).unwrap();
        assert_eq!(bench.state()["busy"], json!(true));
        bench.calls.release();
        bench.pump();
        assert_eq!(bench.state()["busy"], json!(false));
    }
}

// covers: E7-4.8
#[test]
fn stale_turns_true_when_the_target_document_changes_and_false_after_a_refresh() {
    let mut bench = Bench::ready("stale");
    assert_eq!(bench.state()["stale"], json!(false));

    bench.run("file.new", &json!({})).unwrap();
    bench.run("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap();
    assert_eq!(bench.state()["stale"], json!(false), "another tab changed");

    bench.run("doc.switch", &json!({"index": 0})).unwrap();
    bench.run("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap();
    assert_eq!(bench.state()["stale"], json!(true));

    bench.run("playground.refresh", &json!({})).unwrap();
    bench.pump();
    assert_eq!(bench.state()["stale"], json!(false));
}

// covers: E7-4.8
#[test]
fn close_submits_close_and_resets_the_status() {
    let mut bench = Bench::ready("close");

    bench.run("playground.close", &json!({})).unwrap();

    assert_eq!(bench.calls.playground("playground.close").len(), 1);
    assert!(matches!(bench.session.playground_status(), Status::Closed));
    bench.pump();
    let asked = bench.calls.playground(FRAME).len();
    bench.session.playground_poll();
    assert_eq!(bench.calls.playground(FRAME).len(), asked, "an image was asked for a closed playground");
    assert!(matches!(bench.session.playground_status(), Status::Closed));
    assert_eq!(bench.state()["status"], json!("closed"));
}

// covers: E7-4.8
#[test]
fn closing_the_tab_of_the_document_closes_the_playground() {
    let mut bench = Bench::ready("close-tab");

    bench.run("file.close", &json!({"index": 0, "force": true})).unwrap();
    bench.session.pump_capture();

    assert_eq!(bench.calls.playground("playground.close").len(), 1);
    assert!(matches!(bench.session.playground_status(), Status::Closed));
}

// covers: E7-4.8
#[test]
fn a_second_open_closes_the_first() {
    let mut bench = Bench::ready("reopen");

    bench.run("playground.open", &json!({"template": "chat"})).unwrap();
    bench.pump();

    assert_eq!(
        bench.calls.playground_methods().into_iter().filter(|m| m == OPEN || m == "playground.close").collect::<Vec<_>>(),
        [OPEN, "playground.close", OPEN]
    );
    assert!(matches!(bench.session.playground_status(), Status::Ready));
    assert_eq!(bench.state()["template"], json!("chat"));
}

// covers: E7-4.8
#[test]
fn import_and_project_jobs_still_flow_while_the_playground_is_open() {
    let mut bench = Bench::ready("port");

    let added = bench.run("project.addSource", &json!({"package": "synthetic-ui", "version": "1.0.0"})).unwrap();
    let job = added["job"].as_u64().unwrap();
    let listed = bench.session.capture_jobs();
    assert_eq!(listed.iter().map(|j| (j.job, j.command.as_str())).collect::<Vec<_>>(), [(job, "project.addSource")]);
    bench.cycle();
    assert!(matches!(bench.session.playground_status(), Status::Ready));

    bench.calls.release();
    let events = bench.pump();

    let added = events_of(&events, "project.addSource");
    assert_eq!(added.len(), 1, "{events:?}");
    assert!(added[0].outcome.is_ok(), "{:?}", added[0].outcome);
    let manifest: Value = serde_json::from_slice(&std::fs::read(bench.dir.path("uigen.json")).unwrap()).unwrap();
    assert_eq!(manifest["sources"][0]["package"], json!("synthetic-ui"), "{manifest}");
    assert!(matches!(bench.session.playground_status(), Status::Ready));
}
