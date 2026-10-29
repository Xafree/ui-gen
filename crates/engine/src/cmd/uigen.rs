//! UIGEN commands: bringing the output of the capture service into the document.

use serde_json::{Value, json};

use super::CommandSpec;
use crate::params::{f64_of, get, id_of};
use crate::{Result, Session, bad, spec};

const IMPORT_DOM: &str = "uigen.importDom";

pub fn specs() -> Vec<CommandSpec> {
    vec![spec!(
        IMPORT_DOM,
        "Import DOM snapshot",
        "{snapshot (object or JSON text: a DOM snapshot v1 from the UIGEN capture service), parent? (id; default the current page), x?, y? (where the root goes)}  → {root, warnings}",
        true,
        import_dom
    )]
}

fn snapshot_text(p: &Value) -> Result<String> {
    match get(p, "snapshot") {
        Some(Value::String(text)) => Ok(text.clone()),
        Some(object @ Value::Object(_)) => serde_json::to_string(object).map_err(|e| bad(IMPORT_DOM, format!("snapshot: {e}"))),
        _ => Err(bad(IMPORT_DOM, "`snapshot` must be a DOM snapshot object or its JSON text")),
    }
}

fn coordinate(p: &Value, key: &str) -> Result<f64> {
    match get(p, key) {
        None => Ok(0.0),
        Some(v) => f64_of(v).ok_or_else(|| bad(IMPORT_DOM, format!("`{key}` must be a finite number"))),
    }
}

fn parent_of(s: &Session, p: &Value) -> Result<figcraft_doc::NodeId> {
    match get(p, "parent") {
        None => s.page(),
        Some(v) => id_of(v).ok_or_else(|| bad(IMPORT_DOM, "`parent` must be a layer id")),
    }
}

fn import_dom(s: &mut Session, p: &Value) -> Result<Value> {
    let snapshot = figcraft_dom::parse_snapshot(&snapshot_text(p)?).map_err(|e| bad(IMPORT_DOM, e.to_string()))?;
    let parent = parent_of(s, p)?;
    let x = coordinate(p, "x")?;
    let y = coordinate(p, "y")?;
    let imported = figcraft_dom::import_snapshot(s.doc_mut()?, &snapshot, parent, x, y).map_err(|e| bad(IMPORT_DOM, e.to_string()))?;
    Ok(json!({"root": imported.root, "warnings": imported.warnings}))
}
