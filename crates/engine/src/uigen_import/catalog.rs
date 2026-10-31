//! The components of a source library: the answer of the `inspect` job of `uigen.listComponents`,
//! checked and bounded before it is kept in the session, per project folder, package and version.

use std::sync::Arc;

use serde_json::{Value, json};

use super::plan::{MAX_PROPS, MAX_VALUES, usable_value};
use crate::Session;
use crate::capture::{CaptureError, CaptureErrorKind};
use crate::file_access;

pub(crate) const COMMAND: &str = "uigen.listComponents";
const MAX_COMPONENTS: usize = 1000;
const MAX_NAME_CHARS: usize = 128;
const MAX_NOTES: usize = 16;
const MAX_NOTE_CHARS: usize = 256;
const SHOWN_NAME_CHARS: usize = 64;
const UNKNOWN_FRAMEWORK: &str = "unknown";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    Enum,
    Boolean,
    String,
    Number,
    Other,
}

impl PropKind {
    fn parse(name: &str) -> Self {
        match name {
            "enum" => Self::Enum,
            "boolean" => Self::Boolean,
            "string" => Self::String,
            "number" => Self::Number,
            _ => Self::Other,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Enum => "enum",
            Self::Boolean => "boolean",
            Self::String => "string",
            Self::Number => "number",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InspectedProp {
    pub name: String,
    pub kind: PropKind,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InspectedComponent {
    pub name: String,
    pub props: Vec<InspectedProp>,
    pub part_of: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Inspected {
    pub package: String,
    pub version: String,
    pub framework: String,
    pub components: Vec<InspectedComponent>,
    pub notes: Vec<String>,
}

impl Inspected {
    pub(crate) fn to_json(&self) -> Value {
        let components: Vec<Value> = self
            .components
            .iter()
            .map(|component| {
                let props: Vec<Value> =
                    component.props.iter().map(|prop| json!({"name": prop.name, "type": prop.kind.label(), "values": prop.values})).collect();
                match &component.part_of {
                    Some(parent) => json!({"name": component.name, "props": props, "partOf": parent}),
                    None => json!({"name": component.name, "props": props}),
                }
            })
            .collect();
        json!({"package": self.package, "version": self.version, "framework": self.framework, "components": components, "notes": self.notes})
    }
}

struct Entry {
    root: String,
    inspected: Arc<Inspected>,
}

#[derive(Default)]
pub(crate) struct Cache(Vec<Entry>);

impl Cache {
    pub(crate) fn get(&self, root: &str, package: &str, version: &str) -> Option<Arc<Inspected>> {
        let entry = self.0.iter().find(|entry| entry.inspected.package == package && file_access::same_file(&entry.root, root))?;
        (entry.inspected.version == version).then(|| Arc::clone(&entry.inspected))
    }

    fn put(&mut self, root: &str, inspected: Arc<Inspected>) {
        self.forget(root, &inspected.package);
        self.0.push(Entry { root: root.to_string(), inspected });
    }

    pub(crate) fn forget(&mut self, root: &str, package: &str) {
        self.0.retain(|entry| entry.inspected.package != package || !file_access::same_file(&entry.root, root));
    }
}

/// The product of the four axes of an import; an absent style or size axis (0 values) counts for
/// one, and the product saturates.
pub fn render_count(styles: usize, sizes: usize, states: usize, themes: usize) -> usize {
    styles.max(1).saturating_mul(sizes.max(1)).saturating_mul(states).saturating_mul(themes)
}

fn protocol(message: &str) -> CaptureError {
    CaptureError { kind: CaptureErrorKind::Protocol, message: message.to_string() }
}

fn usable_name(name: &str) -> Result<(), &'static str> {
    if name.trim().is_empty() {
        Err("its name is empty")
    } else if name.chars().count() > MAX_NAME_CHARS {
        Err("its name is more than 128 characters")
    } else if name.chars().any(char::is_control) {
        Err("its name has control characters")
    } else {
        Ok(())
    }
}

fn clipped(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

/// A prop with the values an import accepts, once each; a note says how many were left out as unusable.
fn prop_of(listed: &Value, component: &str, notes: &mut Vec<String>) -> Option<InspectedProp> {
    let name = listed.get("name")?.as_str().filter(|name| usable_name(name).is_ok())?;
    let kind = PropKind::parse(listed.get("type").and_then(Value::as_str).unwrap_or_default());
    let mut values: Vec<String> = Vec::new();
    let mut unusable = 0_usize;
    for value in listed.get("values").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        if !usable_value(value) {
            unusable += 1;
        } else if values.len() < MAX_VALUES && !values.iter().any(|seen| seen == value) {
            values.push(value.to_string());
        }
    }
    if unusable > 0 {
        let shown = clipped(component, SHOWN_NAME_CHARS).escape_debug().to_string();
        notes.push(format!("{shown}: {unusable} values of `{name}` were left out (a value has no comma, \"=\", edge space or control character)"));
    }
    Some(InspectedProp { name: name.to_string(), kind, values })
}

fn component_of(listed: &Value, notes: &mut Vec<String>) -> Option<InspectedComponent> {
    let name = listed.get("name").and_then(Value::as_str).unwrap_or_default();
    if let Err(reason) = usable_name(name) {
        notes.push(format!("a component was left out: {reason} (\"{}\")", clipped(name, SHOWN_NAME_CHARS).escape_debug()));
        return None;
    }
    let props = listed
        .get("props")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |props| props.iter().take(MAX_PROPS).filter_map(|prop| prop_of(prop, name, notes)).collect());
    let part_of = listed.get("partOf").and_then(Value::as_str).filter(|parent| usable_name(parent).is_ok()).map(str::to_string);
    Some(InspectedComponent { name: name.to_string(), props, part_of })
}

fn framework_of(result: &Value) -> String {
    let framework = result.get("framework").and_then(Value::as_str);
    framework.filter(|framework| usable_name(framework).is_ok()).map_or_else(|| UNKNOWN_FRAMEWORK.to_string(), str::to_string)
}

/// Check the answer of `inspect` for `package` and bound it; `source_version` is the version the
/// project asks for, used when the service reports none.
fn validate(package: &str, source_version: Option<&str>, result: &Value) -> Result<Inspected, CaptureError> {
    let listed = result.get("components").and_then(Value::as_array).ok_or_else(|| protocol("inspect: the result lists no components"))?;
    let mut notes = Vec::new();
    if listed.len() > MAX_COMPONENTS {
        notes.push(format!("{} components found: the first {MAX_COMPONENTS} are kept", listed.len()));
    }
    let components = listed.iter().take(MAX_COMPONENTS).filter_map(|component| component_of(component, &mut notes)).collect();
    let reported = result
        .get("notes")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |notes| notes.iter().filter_map(Value::as_str).map(str::to_string).collect());
    notes.extend(reported);
    let notes = notes.iter().take(MAX_NOTES).map(|note| clipped(note, MAX_NOTE_CHARS)).collect();
    let version = result.get("version").and_then(Value::as_str).or(source_version).unwrap_or_default().to_string();
    Ok(Inspected { package: package.to_string(), version, framework: framework_of(result), components, notes })
}

#[cfg(test)]
pub(crate) fn validate_for_tests(package: &str, result: &Value) -> Inspected {
    validate(package, None, result).expect("a valid answer")
}

/// The version of the active source `package` in the project of folder `root`, whichever tab holds it.
fn source_version(s: &Session, root: &str, package: &str) -> Option<String> {
    let mut bindings = s.docs.iter().filter_map(|ds| ds.project.as_ref());
    let binding = bindings.find(|binding| file_access::same_file(&binding.root, root))?;
    binding.config.sources.iter().find(|source| source.package == package && !source.removed).map(|source| source.version.clone())
}

/// The final result of a finished `uigen.listComponents` job: the components are kept in the session
/// when they are those of the version the project has now.
pub(crate) fn receive(s: &mut Session, package: &str, root: &str, result: &Value) -> Result<Value, CaptureError> {
    let current = source_version(s, root, package);
    let inspected = validate(package, current.as_deref(), result)?;
    let answer = inspected.to_json();
    if current.as_deref() == Some(inspected.version.as_str()) {
        s.capture.catalog.put(root, Arc::new(inspected));
    }
    Ok(answer)
}

impl Session {
    /// The components of source `package` of the project of the current tab, as the last listing of
    /// its current version found them; `None` before the first listing or after the source changed.
    pub fn inspected(&self, package: &str) -> Option<Arc<Inspected>> {
        let binding = self.project()?;
        let source = binding.config.sources.iter().find(|source| source.package == package && !source.removed)?;
        self.capture.catalog.get(&binding.root, package, &source.version)
    }
}
