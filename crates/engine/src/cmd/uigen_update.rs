//! `uigen.updateSource`: captures again the components imported from a source into the import page of
//! its current version. `uigen.sourceDiff`: what changed since the capture a copy comes from.
//! `uigen.applyDifferences`: takes those differences into the copy.

use std::sync::Arc;

use serde_json::{Value, json};

use super::CommandSpec;
use super::project_sources::{binding, package, require_active_source};
use super::uigen_import::import_component;
use crate::params::{get, req_id};
use crate::project::ProjectBinding;
use crate::uigen_import::{OPTIONS_KEY, imported_sets, move_source_version, page_to_recapture, source_ref};
use crate::uigen_update::{Difference, DifferenceState, Located, MAX_DIFFERENCES, Outcome, SourceDiff, apply, source_diff};
use crate::{Result, Session, bad, spec};

const UPDATE: &str = "uigen.updateSource";
const DIFF: &str = "uigen.sourceDiff";
const APPLY: &str = "uigen.applyDifferences";
const MAX_NAME_CHARS: usize = 128;
const MAX_COMPONENTS: usize = 256;
const MAX_ID_CHARS: usize = 600;
const OPTION_KEYS: [&str; 7] = ["styleProp", "sizeProp", "styles", "sizes", "states", "themes", "animations"];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            UPDATE,
            "Update source components",
            "{package (a source of the library project), components? [names of imported components] (default all)}  captures again every component imported from the source, with the options of its first import, into the import page of the source's current version (\"- Imported · <package> <version>\") and the variable collection \"<package> <version>\"; the components are those of that page, else of the last import page of the package. Returns {jobs: [one import job per component], components: [names]} at once; each import ends with its own event (command uigen.importComponent). The library never changes: see uigen.sourceDiff and uigen.applyDifferences. Refused (`bad`, no job submitted) outside a library project, for a package that is not an active source, when nothing was imported from it, for an unknown component, and without a capture service",
            false,
            update_source
        ),
        spec!(
            DIFF,
            "List source differences",
            "{copy (the component set of the library that uigen.placeImported made)}  what changed between the capture the copy comes from and the one of the source's current version: {copy, name, from (version of the copy), to (current version), base: {page, set} | null (the old capture), latest: {page, set} | null (the new one), differences: [{id (\"<variant>|<path>|<field>\"), variant, path (names of the layers from the root, joined by \" / \"), kind (added|removed|changed), field (layer, or fill|stroke|effects|corner radius|size|position|opacity|blend|visibility|clip content|text|font|auto layout|other), state (clean: the copy still has the old value; modified: the user changed it; unknown: the old capture is gone)}], summary: {clean, modified, unknown}, note?}. A difference the copy already has the new value of is not listed. Without a latest capture, `latest` is null, there is no difference and `note` says to recapture first. At most 2000 differences, `note` says when more. Pure: no job, no write. Refused (`bad`) outside a library project and for a copy that is not a component set with a readable uigen.source",
            false,
            source_differences
        ),
        spec!(
            APPLY,
            "Apply source differences",
            "{copy, ids? [difference ids of uigen.sourceDiff]}  in one undo step, takes into the copy the differences named by `ids`, modified and unknown ones included (the new value replaces the user's); without `ids`, every clean one. An added layer goes in the parent of the same path, a removed layer is removed, a changed field is copied from the latest capture. Returns {applied: [ids], skipped: [{id, reason}] (a modified or unknown difference left alone, or one whose layer is gone from the copy), version (the uigen.source version of the copy)}. When no difference is left the copy's uigen.source version becomes the current one. Refused (`bad`, document untouched) outside a library project, for a copy as in uigen.sourceDiff, and for an id that is not in its list",
            true,
            apply_differences
        ),
    ]
}

fn names(p: &Value) -> Result<Option<Vec<String>>> {
    let refuse = || {
        bad(
            UPDATE,
            format!("`components` must be a list of 1 to {MAX_COMPONENTS} names of {MAX_NAME_CHARS} characters at most, without control characters"),
        )
    };
    let Some(value) = get(p, "components") else { return Ok(None) };
    let items = value.as_array().filter(|items| !items.is_empty() && items.len() <= MAX_COMPONENTS).ok_or_else(refuse)?;
    items
        .iter()
        .map(|item| {
            let name =
                item.as_str().filter(|name| !name.trim().is_empty() && name.chars().count() <= MAX_NAME_CHARS && !name.chars().any(char::is_control));
            name.map(str::to_owned).ok_or_else(refuse)
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

fn import_params(package: &str, component: &str, options: &Value) -> Value {
    let mut params = json!({"package": package, "component": component, "stage": true});
    if let Some(fields) = params.as_object_mut() {
        for key in OPTION_KEYS {
            let kept = options.get(key).filter(|value| !value.is_null() && !value.as_array().is_some_and(Vec::is_empty));
            if let Some(value) = kept {
                fields.insert(key.to_owned(), value.clone());
            }
        }
    }
    params
}

fn options_of(s: &Session, set: figcraft_doc::NodeId, component: &str) -> Result<Value> {
    let kept = s.doc()?.node(set).and_then(|node| node.plugin_data.get(OPTIONS_KEY)).and_then(|raw| serde_json::from_str::<Value>(raw).ok());
    kept.filter(Value::is_object).ok_or_else(|| bad(UPDATE, format!("`{component}` kept no import options: import it again")))
}

fn update_source(s: &mut Session, p: &Value) -> Result<Value> {
    let binding = binding(s, UPDATE)?;
    let package = package(UPDATE, p)?;
    let source = require_active_source(UPDATE, &binding, &package)?;
    let wanted = names(p)?;
    let doc = s.doc()?;
    let page = page_to_recapture(doc, &package, &source.version)
        .ok_or_else(|| bad(UPDATE, format!("nothing is imported from \"{package}\" yet: import a component first")))?;
    let mut components = Vec::new();
    for imported in imported_sets(doc, page) {
        let Some(origin) = source_ref(doc, imported.set) else { continue };
        if wanted.as_ref().is_none_or(|wanted| wanted.contains(&origin.component)) {
            components.push((imported.set, origin.component));
        }
    }
    if let Some(unknown) = wanted.iter().flatten().find(|name| !components.iter().any(|(_, component)| component == *name)) {
        return Err(bad(UPDATE, format!("`{unknown}` is not a component imported from \"{package}\"")));
    }
    let requests = components
        .iter()
        .map(|(set, component)| Ok(import_params(&package, component, &options_of(s, *set, component)?)))
        .collect::<Result<Vec<_>>>()?;
    let mut jobs = Vec::with_capacity(requests.len());
    for request in &requests {
        jobs.push(import_component(s, request)?["job"].clone());
    }
    let listed: Vec<&String> = components.iter().map(|(_, component)| component).collect();
    Ok(json!({"jobs": jobs, "components": listed}))
}

fn located_json(located: Option<Located>) -> Value {
    located.map_or(Value::Null, |located| json!({"page": located.page, "set": located.set}))
}

fn difference_json(difference: &Difference) -> Value {
    json!({
        "id": difference.id,
        "variant": difference.variant,
        "path": difference.path,
        "kind": difference.kind.as_str(),
        "field": difference.field,
        "state": difference.state.as_str(),
    })
}

fn diff_for(s: &Session, binding: &ProjectBinding, cmd: &str, p: &Value) -> Result<SourceDiff> {
    let copy = req_id(cmd, p, "copy")?;
    let doc = s.doc()?;
    let origin = source_ref(doc, copy)
        .ok_or_else(|| bad(cmd, format!("`copy` {copy} has no readable uigen.source: it is not the copy of an imported component")))?;
    let version = binding.config.sources.iter().find(|source| source.package == origin.package).map(|source| source.version.as_str());
    let version = version.ok_or_else(|| bad(cmd, format!("package \"{}\" is not a source of this project", origin.package)))?;
    source_diff(doc, copy, version)
}

fn source_differences(s: &mut Session, p: &Value) -> Result<Value> {
    let binding: Arc<ProjectBinding> = binding(s, DIFF)?;
    let found = diff_for(s, &binding, DIFF, p)?;
    let listed: Vec<Value> = found.differences.iter().map(difference_json).collect();
    let mut result = json!({
        "copy": found.copy,
        "name": found.name,
        "from": found.from,
        "to": found.to,
        "base": located_json(found.base),
        "latest": located_json(found.latest),
        "differences": listed,
        "summary": {"clean": found.summary.clean, "modified": found.summary.modified, "unknown": found.summary.unknown},
    });
    if let (Some(note), Some(fields)) = (&found.note, result.as_object_mut()) {
        fields.insert("note".to_owned(), json!(note));
    }
    Ok(result)
}

fn ids(p: &Value) -> Result<Option<Vec<String>>> {
    let refuse = || bad(APPLY, format!("`ids` must be a list of at most {MAX_DIFFERENCES} difference ids (text)"));
    let Some(value) = get(p, "ids") else { return Ok(None) };
    let items = value.as_array().filter(|items| items.len() <= MAX_DIFFERENCES).ok_or_else(refuse)?;
    items
        .iter()
        .map(|item| item.as_str().filter(|id| id.chars().count() <= MAX_ID_CHARS).map(str::to_owned).ok_or_else(refuse))
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

fn left_alone(difference: &Difference) -> &'static str {
    match difference.state {
        DifferenceState::Modified => "modified in your copy: name its id to take the new value",
        _ => "no earlier capture to tell whether your copy changed it: name its id to take the new value",
    }
}

type Skipped = Vec<(String, String)>;

fn select(found: &SourceDiff, wanted: Option<&[String]>) -> Result<(Vec<Difference>, Skipped)> {
    let Some(wanted) = wanted else {
        let (clean, left): (Vec<&Difference>, Vec<&Difference>) =
            found.differences.iter().partition(|difference| difference.state == DifferenceState::Clean);
        let skipped = left.into_iter().map(|difference| (difference.id.clone(), left_alone(difference).to_owned())).collect();
        return Ok((clean.into_iter().cloned().collect(), skipped));
    };
    if let Some(unknown) = wanted.iter().find(|id| !found.differences.iter().any(|difference| difference.id == **id)) {
        return Err(bad(APPLY, format!("`ids` holds \"{unknown}\", which is not a difference of this copy (see uigen.sourceDiff)")));
    }
    let named = found.differences.iter().filter(|difference| wanted.contains(&difference.id)).cloned().collect();
    Ok((named, Vec::new()))
}

fn apply_differences(s: &mut Session, p: &Value) -> Result<Value> {
    let binding = binding(s, APPLY)?;
    let wanted = ids(p)?;
    let found = diff_for(s, &binding, APPLY, p)?;
    let (chosen, mut skipped) = select(&found, wanted.as_deref())?;
    let outcome = match found.latest {
        Some(latest) if !chosen.is_empty() => apply(s.doc_mut()?, found.copy, latest.set, &chosen),
        _ => Outcome::default(),
    };
    skipped.extend(outcome.skipped);
    let left = found.differences.len().saturating_sub(outcome.applied.len());
    let moved = left == 0 && !found.truncated && found.latest.is_some() && found.from != found.to;
    if moved {
        move_source_version(s.doc_mut()?, found.copy, &found.to)?;
    }
    let skipped: Vec<Value> = skipped.into_iter().map(|(id, reason)| json!({"id": id, "reason": reason})).collect();
    Ok(json!({"applied": outcome.applied, "skipped": skipped, "version": if moved { &found.to } else { &found.from }}))
}
