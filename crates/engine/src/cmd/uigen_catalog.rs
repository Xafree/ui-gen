//! `uigen.listComponents`: the components of a source library, read by the capture service and kept
//! in the session (`crate::uigen_import`). `uigen.importWindow`: asks the app for the import window.

use serde_json::{Value, json};

use super::CommandSpec;
use super::project_sources::{absolute_root, binding, package, require_active_source};
use crate::capture::CaptureJob;
use crate::uigen_import::LIST_COMMAND;
use crate::{Result, Session, bad, spec};

const WINDOW: &str = "uigen.importWindow";
const DIALOG: &str = "dialog.importComponent";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            LIST_COMMAND,
            "List importable components",
            "{package (a source of the library project), refresh? (default false: true asks the capture service again)}  the components the source exports with their props, read by the capture service from the types of the package. Returns {job} at once; the job's final event carries {package, version, framework, components: [{name, props: [{name, type: \"enum\"|\"boolean\"|\"string\"|\"number\"|\"other\", values}]}], notes}. The answer is kept in the session for the version of the source: a second call returns it at once as {cached: true, package, version, framework, components, notes} with no job, until `refresh` or until the source changes or is removed",
            false,
            list_components
        ),
        spec!(
            WINDOW,
            "Import a component…",
            "{package? (a source of the library project: the one the window starts on)}  asks the interface to open the window that imports a component of a source (no dialog from a programmatic call)",
            false,
            import_window
        ),
    ]
}

fn refresh(p: &Value) -> Result<bool> {
    match p.get("refresh") {
        None => Ok(false),
        Some(Value::Bool(on)) => Ok(*on),
        Some(_) => Err(bad(LIST_COMMAND, "`refresh` must be true or false")),
    }
}

fn list_components(s: &mut Session, p: &Value) -> Result<Value> {
    let binding = binding(s, LIST_COMMAND)?;
    let package = package(LIST_COMMAND, p)?;
    let refresh = refresh(p)?;
    require_active_source(LIST_COMMAND, &binding, &package)?;
    if !refresh && let Some(inspected) = s.inspected(&package) {
        let mut answer = inspected.to_json();
        if let Some(fields) = answer.as_object_mut() {
            fields.insert("cached".to_string(), Value::Bool(true));
        }
        return Ok(answer);
    }
    let job = CaptureJob::Inspect { root: absolute_root(LIST_COMMAND, &binding.root)?, package: package.clone() };
    let id = s.submit_capture(LIST_COMMAND, &package, job).map_err(|e| bad(LIST_COMMAND, e.message))?;
    Ok(json!({"job": id}))
}

fn import_window(s: &mut Session, p: &Value) -> Result<Value> {
    let binding = binding(s, WINDOW)?;
    let named = p.get("package").is_some_and(|value| !value.is_null());
    let package = if named {
        let package = package(WINDOW, p)?;
        require_active_source(WINDOW, &binding, &package)?;
        Some(package)
    } else {
        None
    };
    s.request_ui(DIALOG, json!({"package": package}));
    Ok(json!({}))
}
