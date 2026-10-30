//! `uigen.importComponent`: every render job carries a sample text equal to the component name, and
//! the note of a component that takes no content comes out once in the warnings of the import.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::capture::{CaptureBackend, CaptureErrorKind, CaptureJob, CaptureOutcome, JobId};
use crate::uigen_import_support::{Bench, IMPORT, PACKAGE, button_inspect, service_failure, synthetic_button, synthetic_source};

const IGNORED: &str = "sampleText ignored: the component takes no content";

struct Backend {
    note: Option<&'static str>,
    jobs: Arc<Mutex<Vec<CaptureJob>>>,
    ready: Vec<(JobId, CaptureOutcome)>,
}

impl CaptureBackend for Backend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let outcome = match &job {
            CaptureJob::Inspect { .. } => button_inspect(true, true),
            CaptureJob::Render { props, state, theme, .. } => {
                let style = props.get("variant").and_then(Value::as_str).unwrap_or("solid");
                let size = props.get("size").and_then(Value::as_str).unwrap_or("md");
                let mut result = json!({"snapshot": synthetic_button(style, size, state, theme), "png": ".uigen/captures/synthetic.png"});
                if let Some(note) = self.note {
                    result["note"] = json!(note);
                }
                Ok(result)
            }
            CaptureJob::Install { .. } | CaptureJob::Uninstall { .. } => Err(service_failure(CaptureErrorKind::Internal, "unexpected job")),
        };
        let mut jobs = self.jobs.lock().unwrap();
        jobs.push(job);
        let id = u64::try_from(jobs.len()).unwrap();
        self.ready.push((id, outcome));
        id
    }

    fn poll(&mut self) -> Vec<(JobId, CaptureOutcome)> {
        std::mem::take(&mut self.ready)
    }

    fn shutdown(&mut self) {}
}

fn import(tag: &str, note: Option<&'static str>) -> (Value, Vec<CaptureJob>) {
    let jobs = Arc::new(Mutex::new(Vec::new()));
    let mut bench = Bench::without_backend(tag, &[synthetic_source(false)]);
    bench.session.set_capture_backend(Box::new(Backend { note, jobs: Arc::clone(&jobs), ready: Vec::new() }));
    bench.import(&json!({"package": PACKAGE, "component": "Button", "themes": ["light", "dark"]})).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let result = match &event.outcome {
        Ok(result) => result.clone(),
        Err(error) => panic!("the import failed: {}", error.message),
    };
    let submitted = jobs.lock().unwrap().clone();
    (result, submitted)
}

fn warnings(result: &Value) -> Vec<&str> {
    result["warnings"].as_array().unwrap_or_else(|| panic!("no warnings in {result}")).iter().filter_map(Value::as_str).collect()
}

#[test]
fn every_render_job_carries_the_component_name_as_sample_text_loading_included() {
    let (_, jobs) = import("sample-text", None);

    let renders: Vec<(Option<String>, Value)> = jobs
        .into_iter()
        .filter_map(|job| match job {
            CaptureJob::Render { sample_text, props, .. } => Some((sample_text, props)),
            _ => None,
        })
        .collect();
    assert!(!renders.is_empty());
    for (sample_text, props) in &renders {
        assert_eq!(sample_text.as_deref(), Some("Button"), "{props}");
        assert!(props.get("sampleText").is_none(), "the sample text is mixed into the variant props: {props}");
    }
    assert!(renders.iter().any(|(_, props)| props.get("loading") == Some(&json!(true))), "no render is in the Loading state");
}

#[test]
fn the_note_of_a_component_that_takes_no_content_comes_out_once_in_the_warnings() {
    let (result, jobs) = import("sample-note", Some(IGNORED));

    let renders = jobs.iter().filter(|job| matches!(job, CaptureJob::Render { .. })).count();
    assert!(renders > 1, "{renders} renders");
    assert_eq!(warnings(&result).iter().filter(|warning| **warning == IGNORED).count(), 1, "{result}");
}

#[test]
fn no_note_means_no_warning_about_the_sample_text() {
    let (result, _) = import("sample-quiet", None);

    assert!(!warnings(&result).iter().any(|warning| warning.contains("sampleText")), "{result}");
}
