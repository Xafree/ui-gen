//! The token theme of an import: every colour, radius, padding, gap and stroke the translated layers
//! read becomes a variable of the package's collection, with a value per theme mode, and the layer is
//! bound to it. Names follow the CSS variable the page declared, else the role of the value.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry as MapEntry;

use figcraft_doc::{Annotation, Collection, Document, NodeId, NodeKind, Variable, VariableType, VariableValue};
use figcraft_dom::{LayerFacts, NOTE_PREFIX, TokenFact, TokenValue};
use serde_json::json;

use super::pending::COMMAND;
use super::plan::Theme;
use crate::{Result, Session, bad};

const PATH_SEPARATOR: &str = " / ";
const AXES: usize = 3;
const NUMBER_STEP: f64 = 1e-3;

pub(super) struct VariantTokens {
    pub axes: [Option<String>; AXES],
    pub layers: Vec<LayerFacts>,
    pub others: Vec<Vec<LayerFacts>>,
}

pub(super) struct Theming<'a> {
    pub collection: &'a str,
    pub component: &'a str,
    pub added_modes: &'a [String],
    pub themes: &'a [Theme],
}

struct Ctx<'a> {
    owner: &'a Collection,
    themes: &'a [Theme],
    added: &'a [String],
}

const IDENTICAL_DARK: &str = "the dark render is identical to the light one: the library does not follow prefers-color-scheme";

type Slot = (String, String);
type Varying = BTreeMap<String, [bool; AXES]>;
type Seen<'a> = BTreeMap<(&'a str, &'a Slot, [Option<String>; AXES]), &'a [TokenValue]>;

struct Entry {
    variant: usize,
    layer: NodeId,
    slot: Slot,
    exposed: Option<String>,
    role: String,
    kind: VariableType,
    values: Vec<TokenValue>,
}

#[derive(Clone)]
struct Disagreement {
    kept: String,
    imported: String,
}

struct Claim {
    id: String,
    kind: VariableType,
    values: Vec<TokenValue>,
    disagreement: Option<Disagreement>,
}

struct Taken {
    name: String,
    id: String,
    disagreement: Option<Disagreement>,
}

#[derive(Default)]
struct Notes(Vec<(NodeId, String)>);

impl Notes {
    fn add(&mut self, layer: NodeId, message: &str) {
        let note = (layer, format!("{NOTE_PREFIX}{message}"));
        if !self.0.contains(&note) {
            self.0.push(note);
        }
    }
}

/// Bind the layers of every variant to the variables of the collection, creating the ones it lacks and
/// giving the existing ones a value in the modes this import added. `themes` are the modes the facts
/// speak for, the first being the render the layers come from. Returns the warnings to report.
pub(super) fn apply(s: &mut Session, theming: &Theming, variants: &[VariantTokens]) -> Result<Vec<String>> {
    let Theming { collection, component, added_modes, themes } = *theming;
    let mut notes = Notes::default();
    let entries = entries(s.doc()?, component, themes, variants, &mut notes);
    let identical = themes.len() == 2
        && !entries.is_empty()
        && entries.iter().all(|entry| matches!(entry.values.as_slice(), [light, dark] if holds(&stored(*light), dark)));
    let by_role = varying(&entries, variants, |entry| Some(entry.role.as_str()));
    let by_exposed = varying(&entries, variants, |entry| entry.exposed.as_deref());
    let owner = s.doc()?.variables.collection(collection).cloned().ok_or_else(|| bad(COMMAND, "the variable collection is gone"))?;
    let ctx = Ctx { owner: &owner, themes, added: added_modes };

    let mut claims: BTreeMap<String, Claim> = BTreeMap::new();
    let mut bindings: BTreeMap<(String, String), Vec<NodeId>> = BTreeMap::new();
    for entry in &entries {
        let flags = entry.exposed.as_ref().map_or_else(|| by_role.get(&entry.role), |name| by_exposed.get(name));
        let axes = variants.get(entry.variant).map(|variant| &variant.axes);
        let taken = claim(s.doc_mut()?, &ctx, &mut claims, &name_of(entry, flags, axes), entry);
        if let Some(disagreement) = &taken.disagreement {
            notes.add(entry.layer, &format!("{} keeps {}; this render has {}", taken.name, disagreement.kept, disagreement.imported));
        }
        bindings.entry((taken.id, entry.slot.1.clone())).or_default().push(entry.layer);
    }
    for ((variable, field), ids) in bindings {
        s.execute("variable.bind", &json!({"ids": ids, "field": field, "variable": variable}))?;
    }
    let doc = s.doc_mut()?;
    for (layer, label) in notes.0 {
        doc.get_mut(layer)?.annotations.push(Annotation { label, ..Annotation::default() });
    }
    Ok(if identical { vec![IDENTICAL_DARK.to_string()] } else { Vec::new() })
}

fn entries(doc: &Document, component: &str, themes: &[Theme], variants: &[VariantTokens], notes: &mut Notes) -> Vec<Entry> {
    let mut out = Vec::new();
    for (index, variant) in variants.iter().enumerate() {
        let root = variant.layers.first().map_or("", |layer| layer.path.as_str());
        let others: Vec<BTreeMap<&str, &LayerFacts>> =
            variant.others.iter().map(|layers| layers.iter().map(|layer| (layer.path.as_str(), layer)).collect()).collect();
        for layer in &variant.layers {
            let text = doc.node(layer.id).is_some_and(|node| matches!(node.kind, NodeKind::Text(_)));
            for fact in &layer.tokens {
                let Some((family, property)) = role(&fact.field, text) else { continue };
                let first = converted(&fact.field, fact.value);
                let mut values = vec![first];
                for (other, theme) in others.iter().zip(themes.iter().skip(1)) {
                    let found =
                        other.get(layer.path.as_str()).map(|counterpart| counterpart.tokens.iter().find(|candidate| candidate.field == fact.field));
                    values.push(match found {
                        Some(Some(counterpart)) => converted(&fact.field, counterpart.value),
                        _ => {
                            notes.add(layer.id, &missing_note(found.is_some(), &fact.field, *theme, themes));
                            first
                        }
                    });
                }
                let under_root = layer.path.strip_prefix(root).map_or(layer.path.as_str(), |rest| rest.trim_start_matches(PATH_SEPARATOR));
                out.push(entry(index, layer, fact, (family, property), (component, under_root), values));
            }
        }
    }
    out
}

fn entry(
    variant: usize,
    layer: &LayerFacts,
    fact: &TokenFact,
    (family, property): (&str, &str),
    (component, under_root): (&str, &str),
    values: Vec<TokenValue>,
) -> Entry {
    Entry {
        variant,
        layer: layer.id,
        slot: (layer.path.clone(), fact.field.clone()),
        exposed: fact.exposed.as_deref().map(|name| name.trim_start_matches('-')).filter(|name| !name.is_empty()).map(str::to_owned),
        role: role_name(family, component, under_root, property),
        kind: if matches!(fact.value, TokenValue::Color(_)) { VariableType::Color } else { VariableType::Number },
        values,
    }
}

fn missing_note(layer_exists: bool, field: &str, theme: Theme, themes: &[Theme]) -> String {
    let kept = themes.first().map_or("", |first| first.mode());
    let what = if layer_exists { format!("no {field}") } else { "no such layer".to_string() };
    format!("the {} render has {what}: {} keeps the {kept} value", theme.render(), theme.mode())
}

/// An opacity variable holds a percentage.
fn converted(field: &str, value: TokenValue) -> TokenValue {
    match value {
        TokenValue::Number(number) if field == "opacity" => TokenValue::Number(number * 100.0),
        other => other,
    }
}

fn role(field: &str, text: bool) -> Option<(&'static str, &'static str)> {
    Some(match field {
        "fill" if text => ("couleur", "texte"),
        "fill" => ("couleur", "fond"),
        "stroke" => ("couleur", "bordure"),
        "strokeWeight" => ("bordure", "epaisseur"),
        "topLeftRadius" | "topRightRadius" | "bottomRightRadius" | "bottomLeftRadius" => ("rayon", "rayon"),
        "paddingTop" | "paddingBottom" => ("espace", "padding-y"),
        "paddingLeft" | "paddingRight" => ("espace", "padding-x"),
        "gap" => ("espace", "gap"),
        "opacity" => ("opacite", "opacite"),
        "fontSize" => ("typo", "taille"),
        _ if field.starts_with("effects/") && field.ends_with("/color") => ("couleur", "ombre"),
        _ => return None,
    })
}

fn kebab(text: &str) -> String {
    let words: Vec<String> = text.split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()).map(str::to_lowercase).collect();
    if words.is_empty() { "layer".to_string() } else { words.join("-") }
}

fn role_name(family: &str, component: &str, under_root: &str, property: &str) -> String {
    let mut segments = vec![family.to_string(), kebab(component)];
    if !under_root.is_empty() {
        segments.extend(under_root.split(PATH_SEPARATOR).map(kebab));
    }
    segments.push(property.to_string());
    segments.join("/")
}

/// For every key, the axes whose value changes it: two variants that differ in that axis only give
/// the same layer and field another value.
fn varying<'a>(entries: &'a [Entry], variants: &[VariantTokens], key: impl Fn(&'a Entry) -> Option<&'a str>) -> Varying {
    let mut found = Varying::new();
    for axis in 0..AXES {
        let mut seen = Seen::new();
        for entry in entries {
            let (Some(key), Some(variant)) = (key(entry), variants.get(entry.variant)) else { continue };
            let mut axes = variant.axes.clone();
            if let Some(masked) = axes.get_mut(axis) {
                *masked = Some(String::new());
            }
            match seen.entry((key, &entry.slot, axes)) {
                MapEntry::Vacant(vacant) => {
                    vacant.insert(&entry.values);
                }
                MapEntry::Occupied(occupied) if !same_values(occupied.get(), &entry.values) => {
                    if let Some(flag) = found.entry(key.to_owned()).or_default().get_mut(axis) {
                        *flag = true;
                    }
                }
                MapEntry::Occupied(_) => {}
            }
        }
    }
    found
}

/// The exposed name, else the role, then the values of the axes that change it: Style, Size, State.
fn name_of(entry: &Entry, varying: Option<&[bool; AXES]>, axes: Option<&[Option<String>; AXES]>) -> String {
    let mut name = entry.exposed.clone().unwrap_or_else(|| entry.role.clone());
    if let (Some(varying), Some(axes)) = (varying, axes) {
        for (changes, value) in varying.iter().zip(axes) {
            if let (true, Some(value)) = (changes, value) {
                name.push('/');
                name.push_str(&kebab(value));
            }
        }
    }
    name
}

/// The variable `entry` binds to. Within an import a name holds one set of values: an entry with
/// other values takes the next free name. A variable the collection already has is never changed,
/// even when this render disagrees with it.
fn claim(doc: &mut Document, ctx: &Ctx, claims: &mut BTreeMap<String, Claim>, name: &str, entry: &Entry) -> Taken {
    let mut number = 1_u32;
    loop {
        let candidate = if number == 1 { name.to_owned() } else { format!("{name} {number}") };
        number += 1;
        if let Some(claim) = claims.get(&candidate) {
            if claim.kind == entry.kind && same_values(&claim.values, &entry.values) {
                return Taken { name: candidate, id: claim.id.clone(), disagreement: claim.disagreement.clone() };
            }
            continue;
        }
        let existing = doc.variables.variables.iter_mut().find(|variable| variable.collection == ctx.owner.id && variable.name == candidate);
        let claim = match existing {
            Some(variable) if variable.kind == entry.kind => {
                fill(variable, ctx, &entry.values, true);
                Claim {
                    id: variable.id.clone(),
                    kind: entry.kind,
                    values: entry.values.clone(),
                    disagreement: disagreement(variable, ctx.owner, ctx.themes, &entry.values),
                }
            }
            Some(_) => continue,
            None => Claim { id: create(doc, ctx, &candidate, entry), kind: entry.kind, values: entry.values.clone(), disagreement: None },
        };
        let taken = Taken { name: candidate.clone(), id: claim.id.clone(), disagreement: claim.disagreement.clone() };
        claims.insert(candidate, claim);
        return taken;
    }
}

/// Give `variable` the value of each theme in the mode of that name: only in the modes this import added
/// when `only_added` (the other modes hold the variable's own values), else in every mode a theme names.
fn fill(variable: &mut Variable, ctx: &Ctx, values: &[TokenValue], only_added: bool) {
    for (theme, value) in ctx.themes.iter().zip(values) {
        let mode = ctx.owner.modes.iter().find(|mode| mode.name == theme.mode() && (!only_added || ctx.added.contains(&mode.id)));
        if let Some(mode) = mode {
            variable.values.insert(mode.id.clone(), stored(*value));
        }
    }
}

fn create(doc: &mut Document, ctx: &Ctx, name: &str, entry: &Entry) -> String {
    let id = doc.new_variable_id();
    let mut variable = Variable {
        id: id.clone(),
        name: name.to_owned(),
        collection: ctx.owner.id.clone(),
        kind: entry.kind,
        values: Default::default(),
        description: String::new(),
        scopes: None,
        hidden_from_publishing: false,
        extends: None,
        overrides: Default::default(),
        code_syntax: Default::default(),
    };
    fill(&mut variable, ctx, &entry.values, false);
    doc.variables.variables.push(variable);
    id
}

/// What an existing variable holds in the modes this import has values for, when it is not the same.
fn disagreement(variable: &Variable, owner: &Collection, themes: &[Theme], values: &[TokenValue]) -> Option<Disagreement> {
    let (mut kept, mut imported) = (Vec::new(), Vec::new());
    for (theme, value) in themes.iter().zip(values) {
        let Some(held) = owner.modes.iter().find(|mode| mode.name == theme.mode()).and_then(|mode| variable.values.get(&mode.id)) else { continue };
        if !holds(held, value) {
            kept.push(format!("{} {}", theme.mode(), shown_variable(held)));
            imported.push(format!("{} {}", theme.mode(), shown(*value)));
        }
    }
    (!kept.is_empty()).then(|| Disagreement { kept: kept.join(", "), imported: imported.join(", ") })
}

fn stored(value: TokenValue) -> VariableValue {
    match value {
        TokenValue::Color(color) => VariableValue::Color(color),
        TokenValue::Number(number) => VariableValue::Number(number),
    }
}

fn same_values(a: &[TokenValue], b: &[TokenValue]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| holds(&stored(*a), b))
}

fn holds(held: &VariableValue, value: &TokenValue) -> bool {
    match (held, value) {
        (VariableValue::Color(held), TokenValue::Color(value)) => held.to_rgba8() == value.to_rgba8(),
        (VariableValue::Number(held), TokenValue::Number(value)) => (held - value).abs() <= NUMBER_STEP,
        _ => false,
    }
}

fn shown(value: TokenValue) -> String {
    shown_variable(&stored(value))
}

fn shown_variable(value: &VariableValue) -> String {
    match value {
        VariableValue::Color(color) => color.to_css_hex(),
        VariableValue::Number(number) => format!("{}", (number * 100.0).round() / 100.0),
        VariableValue::String(text) => text.clone(),
        VariableValue::Boolean(flag) => flag.to_string(),
        VariableValue::Alias(id) => id.clone(),
    }
}
