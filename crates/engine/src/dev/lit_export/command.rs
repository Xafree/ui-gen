//! The commands of the Lit export: `export.lit` plans the package and `export.litWindow` asks the
//! app to open the window that drives it.

use figcraft_doc::{Document, NodeId, NodeKind};
use serde_json::{Value, json};

use super::package::{valid_name, valid_version};
use super::{ExportOptions, plan, write};
use crate::cmd::CommandSpec;
use crate::lit::{LitOptions, valid_prefix};
use crate::params::{get, id_of};
use crate::{Result, bad, spec};

const CMD: &str = "export.lit";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "export.lit",
            "Export library as Lit",
            "{ids? (components and sets; default all), prefix?: \"ui-\" (custom element names), tokenPrefix? (custom properties; default the prefix without hyphens), collections? (names or ids; default all), lightMode?, darkMode?, packageName?: \"uigen-lib\", version?: \"0.1.0\", path? (folder to write the package to), force?: false (replace files that were edited by hand or not generated)} → {ready, components: [{id, name, tag, className, file, status: ok|empty|invalid|dependency, issues: [{code: empty|leadingDigit|duplicate|reserved|noHyphen|tooLong, message}]}], files: [{path, kind, bytes, text (without `path`)}], notes, written?, unchanged?, kept?, skipped?: [{path, reason: edited|not-generated}], stale?}. Without `path` nothing is written; `path` is refused while a component has an issue, and keeps `.uigen/export.json`, a manifest of the generated files, to tell its files from the ones you add",
            false,
            |s, p| run(s.doc()?, p)
        ),
        spec!("export.litWindow", "Export library as Lit…", "{}  opens the export window", false, |s, _| {
            s.request_ui("export.lit", json!({}));
            crate::cmd::ok()
        }),
    ]
}

fn text(p: &Value, key: &str) -> Result<Option<String>> {
    match get(p, key) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(bad(CMD, format!("`{key}` must be a string"))),
    }
}

fn prefix(p: &Value, key: &str) -> Result<Option<String>> {
    match text(p, key)? {
        Some(prefix) if !valid_prefix(&prefix) => Err(bad(CMD, format!("`{key}`: lowercase letters, digits and hyphens (up to 20)"))),
        other => Ok(other),
    }
}

fn components(doc: &Document, p: &Value) -> Result<Option<Vec<NodeId>>> {
    let Some(list) = get(p, "ids") else { return Ok(None) };
    let Value::Array(list) = list else { return Err(bad(CMD, "`ids` must be an array of component or set ids")) };
    let mut ids = Vec::new();
    for v in list {
        let id = id_of(v).ok_or_else(|| bad(CMD, "`ids`: every entry must be a node id"))?;
        if !doc.node(id).is_some_and(|n| matches!(n.kind, NodeKind::Component { .. } | NodeKind::ComponentSet { .. })) {
            return Err(bad(CMD, format!("`ids`: {id} is not a component or a component set")));
        }
        ids.push(id);
    }
    Ok(Some(ids))
}

fn collections(doc: &Document, p: &Value) -> Result<Option<Vec<String>>> {
    let Some(list) = get(p, "collections") else { return Ok(None) };
    let Value::Array(list) = list else { return Err(bad(CMD, "`collections` must be an array of collection names")) };
    let mut ids = Vec::new();
    for v in list {
        let found = v.as_str().and_then(|name| doc.variables.collection(name));
        ids.push(found.ok_or_else(|| bad(CMD, "`collections`: every entry must name a variable collection"))?.id.clone());
    }
    Ok(Some(ids))
}

fn options(doc: &Document, p: &Value) -> Result<ExportOptions> {
    let mut out = ExportOptions { ids: components(doc, p)?, collections: collections(doc, p)?, ..ExportOptions::default() };
    if let Some(prefix) = prefix(p, "prefix")? {
        out.lit = LitOptions::new(&prefix);
    }
    if let Some(tokens) = prefix(p, "tokenPrefix")? {
        out.lit.token_prefix = tokens;
    }
    (out.light, out.dark) = (text(p, "lightMode")?, text(p, "darkMode")?);
    if let Some(name) = text(p, "packageName")? {
        if !valid_name(&name) {
            return Err(bad(CMD, "`packageName`: a lowercase npm package name, scoped (`@scope/name`) or not"));
        }
        out.package_name = name;
    }
    if let Some(version) = text(p, "version")? {
        if !valid_version(&version) {
            return Err(bad(CMD, "`version`: a semantic version such as 1.2.3"));
        }
        out.version = version;
    }
    Ok(out)
}

fn flag(p: &Value, key: &str) -> Result<bool> {
    match get(p, key) {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(bad(CMD, format!("`{key}` must be true or false"))),
    }
}

fn folder(p: &Value) -> Result<Option<String>> {
    match text(p, "path")? {
        Some(dir) if dir.trim().is_empty() => Err(bad(CMD, "`path` must name a folder")),
        other => Ok(other),
    }
}

fn run(doc: &Document, p: &Value) -> Result<Value> {
    let plan = plan(doc, &options(doc, p)?);
    let force = flag(p, "force")?;
    let report = match folder(p)? {
        None => None,
        Some(_) if !plan.ready() => {
            return Err(bad(CMD, format!("nothing was written; fix these components first: {}", plan.blockers().join(", "))));
        }
        Some(dir) => Some(write(&plan, &dir, force)?),
    };
    let files: Vec<Value> = plan
        .files
        .iter()
        .map(|f| match report {
            None => json!({"path": f.path, "kind": f.kind, "bytes": f.text.len(), "text": f.text}),
            Some(_) => json!({"path": f.path, "kind": f.kind, "bytes": f.text.len()}),
        })
        .collect();
    let mut out = json!({"ready": plan.ready(), "components": plan.components, "files": files});
    let mut notes = plan.notes;
    if let (Some(report), Some(all)) = (report, out.as_object_mut()) {
        notes.extend(report.notes.iter().cloned());
        if let Ok(Value::Object(done)) = serde_json::to_value(&report) {
            all.extend(done);
        }
    }
    out["notes"] = json!(notes);
    Ok(out)
}
