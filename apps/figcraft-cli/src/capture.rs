//! The capture service (Node sidecar) for the headless commands that change a project's source
//! libraries: `run` waits for the jobs its steps submitted, `mcp` applies them as calls come in.

use std::time::Duration;

use figcraft_engine::Session;
use serde_json::{Value, json};

const POLL: Duration = Duration::from_millis(100);
const SOURCE_COMMANDS: [&str; 3] = ["project.addSource", "project.setSourceVersion", "project.removeSource"];

pub fn needs_service(commands: &[&str]) -> bool {
    commands.iter().any(|command| SOURCE_COMMANDS.contains(command))
}

/// Give `session` the capture service. Without Node or the sidecar the commands that need it
/// report it is unavailable; the reason goes to stderr once.
pub fn install(session: &mut Session) {
    match figcraft_capture::locate() {
        Ok(config) => session.set_capture_backend(Box::new(figcraft_capture::CaptureService::start(config, Box::new(|| {})))),
        Err(e) => eprintln!("capture service not started: {e}"),
    }
}

fn working(session: &mut Session) -> bool {
    let Ok(status) = session.execute("capture.status", &json!({})) else { return false };
    status
        .get("jobs")
        .and_then(Value::as_array)
        .is_some_and(|jobs| jobs.iter().any(|job| matches!(job.get("state").and_then(Value::as_str), Some("queued" | "running"))))
}

/// Wait for every job submitted so far and apply the results; the failures come back as messages.
pub fn finish(session: &mut Session) -> Vec<String> {
    let mut failures = Vec::new();
    loop {
        for event in session.pump_capture() {
            if let Err(e) = event.outcome {
                failures.push(format!("{} {}: {}", event.command, event.package, e.message));
            }
        }
        if !working(session) {
            return failures;
        }
        std::thread::sleep(POLL);
    }
}
