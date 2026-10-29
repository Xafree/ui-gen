//! UIGEN commands: bringing the output of the capture service into the document.

use figcraft_doc::NodeKind;
use serde_json::{Value, json};

use super::CommandSpec;
use crate::params::{f64_of, get, id_of};
use crate::{Result, Session, bad, spec};

const IMPORT_DOM: &str = "uigen.importDom";

pub fn specs() -> Vec<CommandSpec> {
    vec![spec!(
        IMPORT_DOM,
        "Import DOM snapshot",
        "{snapshot (object or JSON text: a DOM snapshot v1 from the UIGEN capture service), parent? (a frame or page id; default the current page), x?, y? (where the root goes)}  → {root, warnings}",
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
    let Some(v) = get(p, "parent") else { return s.page() };
    let id = id_of(v).ok_or_else(|| bad(IMPORT_DOM, "`parent` must be a layer id"))?;
    let node = s.doc()?.node(id).ok_or_else(|| bad(IMPORT_DOM, format!("`parent` {id} does not exist")))?;
    if node.is_page() || matches!(node.kind, NodeKind::Frame | NodeKind::Component { .. }) {
        return Ok(id);
    }
    Err(bad(IMPORT_DOM, format!("`parent` {id} is a {}: only a frame or a page can receive imported layers", node.kind.type_name())))
}

fn import_dom(s: &mut Session, p: &Value) -> Result<Value> {
    let snapshot = figcraft_dom::parse_snapshot(&snapshot_text(p)?).map_err(|e| bad(IMPORT_DOM, e.to_string()))?;
    let parent = parent_of(s, p)?;
    let x = coordinate(p, "x")?;
    let y = coordinate(p, "y")?;
    figcraft_text::scan_system_fonts();
    let imported = figcraft_dom::import_snapshot(s.doc_mut()?, &snapshot, parent, x, y).map_err(|e| bad(IMPORT_DOM, e.to_string()))?;
    Ok(json!({"root": imported.root, "warnings": imported.warnings}))
}
