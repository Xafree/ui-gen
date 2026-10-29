//! UIGEN commands: bringing the output of the capture service into the document.

use figcraft_doc::NodeKind;
use serde_json::{Value, json};

use super::CommandSpec;
use crate::params::{bool_of, f64_of, get, id_of};
use crate::{Result, Session, bad, spec};

const IMPORT_DOM: &str = "uigen.importDom";

pub fn specs() -> Vec<CommandSpec> {
    vec![spec!(
        IMPORT_DOM,
        "Import DOM snapshot",
        "{snapshot (object or JSON text: a DOM snapshot v1 from the UIGEN capture service), parent? (a frame or page id; default the current page), x?, y? (where the root goes), full? (true: auto layout from flex, empty wrappers dropped, unique names, a note on each layer for each warning)}  → {root, warnings}",
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

fn full_translation(p: &Value) -> Result<bool> {
    match get(p, "full") {
        None => Ok(false),
        Some(v) => bool_of(v).ok_or_else(|| bad(IMPORT_DOM, "`full` must be true or false")),
    }
}

fn import_dom(s: &mut Session, p: &Value) -> Result<Value> {
    let snapshot = figcraft_dom::parse_snapshot(&snapshot_text(p)?).map_err(|e| bad(IMPORT_DOM, e.to_string()))?;
    let parent = parent_of(s, p)?;
    let x = coordinate(p, "x")?;
    let y = coordinate(p, "y")?;
    let full = full_translation(p)?;
    figcraft_text::scan_system_fonts();
    let doc = s.doc_mut()?;
    let (root, warnings) = if full {
        let translated = figcraft_dom::translate_snapshot(doc, &snapshot, parent, x, y, &figcraft_dom::TranslateOptions::default())
            .map_err(|e| bad(IMPORT_DOM, e.to_string()))?;
        (translated.root, translated.warnings)
    } else {
        let imported = figcraft_dom::import_snapshot(doc, &snapshot, parent, x, y).map_err(|e| bad(IMPORT_DOM, e.to_string()))?;
        (imported.root, imported.warnings)
    };
    Ok(json!({"root": root, "warnings": warnings}))
}
