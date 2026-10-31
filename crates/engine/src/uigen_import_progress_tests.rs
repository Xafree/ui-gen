use std::sync::{Arc, Mutex};

use serde_json::json;

use crate::capture::{CaptureBackend, CaptureErrorKind, CaptureJob, CaptureOutcome, JobId};
use crate::uigen_import_support::{Bench, PACKAGE, button_inspect, service_failure, synthetic_button, synthetic_source};
use crate::{ImportProgress, Session};

#[derive(Default)]
struct Held {
    ready: Vec<(JobId, CaptureOutcome)>,
    submitted: u64,
    allowed: usize,
    failing_render: bool,
}

struct HeldBackend(Arc<Mutex<Held>>);

impl CaptureBackend for HeldBackend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let mut held = self.0.lock().unwrap();
        let outcome = match job {
            CaptureJob::Inspect { .. } => button_inspect(true, false),
            _ if held.failing_render => Err(service_failure(CaptureErrorKind::Render, "render failed")),
            _ => Ok(json!({"snapshot": synthetic_button("solid", "md", "default", "light"), "png": ".uigen/captures/synthetic.png"})),
        };
        held.submitted += 1;
        let id = held.submitted;
        held.ready.push((id, outcome));
        id
    }

    fn poll(&mut self) -> Vec<(JobId, CaptureOutcome)> {
        let mut held = self.0.lock().unwrap();
        let count = held.allowed.min(held.ready.len());
        held.allowed -= count;
        held.ready.drain(..count).collect()
    }

    fn shutdown(&mut self) {}
}

fn started(tag: &str) -> (Bench, Arc<Mutex<Held>>, JobId) {
    let mut bench = Bench::without_backend(tag, &[synthetic_source(false)]);
    let held = Arc::new(Mutex::new(Held::default()));
    bench.session.set_capture_backend(Box::new(HeldBackend(Arc::clone(&held))));
    let params = json!({
        "package": PACKAGE,
        "component": "Button",
        "styleProp": "variant",
        "styles": ["solid", "outline"],
        "sizeProp": "size",
        "sizes": ["sm", "md"],
        "states": ["Default", "Hover"],
        "themes": ["light", "dark"],
        "stage": true,
    });
    let head = bench.import(&params).unwrap()["job"].as_u64().unwrap();
    (bench, held, head)
}

fn answer(session: &mut Session, held: &Arc<Mutex<Held>>, count: usize) {
    held.lock().unwrap().allowed += count;
    session.pump_capture();
}

#[test]
fn the_total_is_absent_until_the_inspection_is_back() {
    let (bench, _, head) = started("progress-inspect");
    assert_eq!(bench.session.import_progress(head), Some(ImportProgress { rendered: 0, total: None }));
}

#[test]
fn each_render_back_counts_one_and_the_total_is_the_number_of_renders_submitted() {
    let (mut bench, held, head) = started("progress-count");
    answer(&mut bench.session, &held, 1);
    let total = usize::try_from(held.lock().unwrap().submitted).unwrap() - 1;
    assert_eq!(total, 16, "2 styles x 2 sizes x 2 states x 2 themes");
    assert_eq!(bench.session.import_progress(head), Some(ImportProgress { rendered: 0, total: Some(total) }));
    for rendered in 1..total {
        answer(&mut bench.session, &held, 1);
        assert_eq!(bench.session.import_progress(head), Some(ImportProgress { rendered, total: Some(total) }));
    }
}

#[test]
fn an_import_that_ended_or_never_ran_has_no_progress() {
    let (mut bench, held, head) = started("progress-end");
    assert_eq!(bench.session.import_progress(head + 1), None, "the id of no import");
    answer(&mut bench.session, &held, 1);
    answer(&mut bench.session, &held, 16);
    assert_eq!(bench.session.import_progress(head), None);
}

#[test]
fn a_failed_render_ends_the_progress() {
    let (mut bench, held, head) = started("progress-failed");
    held.lock().unwrap().failing_render = true;
    answer(&mut bench.session, &held, 1);
    answer(&mut bench.session, &held, 1);
    assert_eq!(bench.session.import_progress(head), None);
}
