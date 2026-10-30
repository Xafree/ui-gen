//! Token themes: the local variable collections of a library project, and the rebinding of the
//! layers of the library from one collection to another by variable name and type.

use std::collections::{BTreeMap, HashMap, HashSet};

use figcraft_doc::{Collection, Document, NodeId, Variable, VariableType, VariableValue};
use figcraft_project::Project;

use crate::Result;
use crate::library::local_collections;
use crate::project::{component_pages, component_roots};
use crate::uigen_import::sections;

pub const MAX_SWATCHES: usize = 8;
pub const MAX_UNMATCHED: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Swatch {
    pub name: String,
    pub light: String,
    pub dark: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeInfo {
    pub collection: String,
    pub name: String,
    pub origin: Option<(String, String)>,
    pub modes: Vec<String>,
    pub variables: usize,
    pub bound_layers: usize,
    pub swatches: Vec<Swatch>,
}

pub struct Rebinding {
    changes: Vec<(NodeId, String, String)>,
    pub layers: usize,
    pub unmatched: Vec<String>,
}

impl Rebinding {
    pub fn rebound(&self) -> usize {
        self.changes.len()
    }
}

pub fn themes(doc: &Document, project: &Project) -> Vec<ThemeInfo> {
    let followers = followers(doc);
    let origins: HashMap<String, (String, String)> = sections(doc, project)
        .into_iter()
        .map(|section| (format!("{} {}", section.package, section.version), (section.package, section.version)))
        .collect();
    local_collections(doc)
        .map(|collection| {
            let variables: Vec<&Variable> = doc.variables.variables.iter().filter(|variable| variable.collection == collection.id).collect();
            ThemeInfo {
                collection: collection.id.clone(),
                name: collection.name.clone(),
                origin: origins.get(&collection.name).cloned(),
                modes: collection.modes.iter().map(|mode| mode.name.clone()).collect(),
                variables: variables.len(),
                bound_layers: followers.get(&collection.id).copied().unwrap_or(0),
                swatches: swatches(doc, collection, &variables),
            }
        })
        .collect()
}

pub fn most_followed(doc: &Document) -> Option<String> {
    let followers = followers(doc);
    let mut best: Option<(&Collection, usize)> = None;
    for collection in local_collections(doc) {
        let count = followers.get(&collection.id).copied().unwrap_or(0);
        if count > best.map_or(0, |(_, most)| most) {
            best = Some((collection, count));
        }
    }
    best.map(|(collection, _)| collection.name.clone())
}

pub fn rebinding(doc: &Document, target: &str) -> Rebinding {
    let variables: HashMap<&str, &Variable> = doc.variables.variables.iter().map(|variable| (variable.id.as_str(), variable)).collect();
    let mut equivalents: HashMap<(String, &'static str), &str> = HashMap::new();
    for variable in doc.variables.variables.iter().filter(|variable| variable.collection == target) {
        equivalents.entry((key(&variable.name), variable.kind.id())).or_insert(variable.id.as_str());
    }
    let mut changes = Vec::new();
    let mut changed_layers = HashSet::new();
    let mut unmatched = Vec::new();
    let mut seen = HashSet::new();
    for layer in library_layers(doc) {
        let Some(node) = doc.node(layer) else { continue };
        for (field, bound) in &node.bound {
            let Some(variable) = variables.get(bound.as_str()).filter(|variable| variable.collection != target) else { continue };
            match equivalents.get(&(key(&variable.name), variable.kind.id())) {
                Some(equivalent) => {
                    changes.push((layer, field.clone(), (*equivalent).to_string()));
                    changed_layers.insert(layer);
                }
                None if seen.insert(variable.name.clone()) => unmatched.push(variable.name.clone()),
                None => {}
            }
        }
    }
    unmatched.truncate(MAX_UNMATCHED);
    Rebinding { changes, layers: changed_layers.len(), unmatched }
}

pub fn rebind(doc: &mut Document, plan: &Rebinding) -> Result<()> {
    for (layer, field, variable) in &plan.changes {
        doc.get_mut(*layer)?.bound.insert(field.clone(), variable.clone());
    }
    Ok(())
}

fn key(name: &str) -> String {
    name.trim().to_lowercase()
}

fn library_layers(doc: &Document) -> Vec<NodeId> {
    component_pages(doc).iter().flat_map(|info| component_roots(doc, info.page)).flat_map(|root| doc.subtree(root)).collect()
}

fn followers(doc: &Document) -> HashMap<String, usize> {
    let owners: HashMap<&str, &str> = doc.variables.variables.iter().map(|variable| (variable.id.as_str(), variable.collection.as_str())).collect();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for layer in library_layers(doc) {
        let Some(node) = doc.node(layer) else { continue };
        let collections: HashSet<&str> = node.bound.values().filter_map(|bound| owners.get(bound.as_str()).copied()).collect();
        for collection in collections {
            *counts.entry(collection.to_string()).or_default() += 1;
        }
    }
    counts
}

fn swatches(doc: &Document, collection: &Collection, variables: &[&Variable]) -> Vec<Swatch> {
    let named = |wanted: &str| collection.modes.iter().find(|mode| mode.name.trim().eq_ignore_ascii_case(wanted));
    let light = named("light").or(collection.modes.first());
    let dark = named("dark").or(collection.modes.get(1)).or(collection.modes.first());
    let (Some(light), Some(dark)) = (light, dark) else { return Vec::new() };
    let color_in =
        |variable: &Variable, mode: &str| match doc.variables.resolve(&variable.id, &BTreeMap::from([(collection.id.clone(), mode.to_string())])) {
            Some(VariableValue::Color(color)) => Some(format!("#{}", color.to_hex())),
            _ => None,
        };
    variables
        .iter()
        .filter(|variable| variable.kind == VariableType::Color)
        .filter_map(|variable| {
            Some(Swatch { name: variable.name.clone(), light: color_in(variable, &light.id)?, dark: color_in(variable, &dark.id)? })
        })
        .take(MAX_SWATCHES)
        .collect()
}
