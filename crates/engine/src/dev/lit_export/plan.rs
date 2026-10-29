//! The plan of a Lit export: which components become files, under which element names and with
//! which problems, and the text of every file of the package. Nothing is written here.

use std::collections::{BTreeMap, HashSet};

use figcraft_doc::{Collection, Document, NodeId, NodeKind};
use serde::Serialize;

use super::{TokenOptions, package, tokens_css};
use crate::component_codegen::{Component, ParamKind, describe, main_of};
use crate::components::variant_set;
use crate::lit::{self, LitOptions, Note, StateRecipe, TagIssue};
use crate::params::id_of;

const MAX_VISITED: usize = 1_000_000;
const MAX_SHOWN_NAME: usize = 60;

/// What to export, and how the package is named.
#[derive(Clone, Debug)]
pub struct ExportOptions {
    /// Components and sets to export; every one of the document when `None`.
    pub ids: Option<Vec<NodeId>>,
    pub lit: LitOptions,
    /// Ids of the variable collections `tokens.css` declares; all of them when `None`.
    pub collections: Option<Vec<String>>,
    pub light: Option<String>,
    pub dark: Option<String>,
    pub package_name: String,
    pub version: String,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            ids: None,
            lit: LitOptions::default(),
            collections: None,
            light: None,
            dark: None,
            package_name: package::DEFAULT_NAME.to_string(),
            version: package::DEFAULT_VERSION.to_string(),
        }
    }
}

/// Where a component stands in the export.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Empty,
    Invalid,
    Dependency,
}

/// Why a component can't be exported as it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub code: &'static str,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedComponent {
    /// The component, or the set a variant belongs to.
    pub id: NodeId,
    pub name: String,
    pub tag: Option<String>,
    pub class_name: Option<String>,
    pub file: Option<String>,
    pub status: Status,
    pub issues: Vec<Issue>,
    /// One recipe per variant: how a harness puts the element in that variant's state.
    pub states: Vec<StateRecipe>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedFile {
    pub path: String,
    pub kind: &'static str,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportPlan {
    pub components: Vec<PlannedComponent>,
    /// Sorted by path.
    pub files: Vec<PlannedFile>,
    pub notes: Vec<Note>,
}

impl ExportPlan {
    /// Whether the package can be written: no component has an issue.
    pub fn ready(&self) -> bool {
        self.components.iter().all(|c| c.issues.is_empty())
    }

    /// The components that stand in the way of a write, with what is wrong.
    pub fn blockers(&self) -> Vec<String> {
        self.components
            .iter()
            .filter(|c| !c.issues.is_empty())
            .map(|c| format!("{} ({})", shown(&c.name), c.issues.iter().map(|i| i.code).collect::<Vec<_>>().join(", ")))
            .collect()
    }
}

struct Candidate {
    id: NodeId,
    name: String,
    comp: Component,
    empty: bool,
    needed_by: Option<(String, String)>,
}

fn shown(name: &str) -> String {
    let cut: String = name.chars().take(MAX_SHOWN_NAME).collect();
    if cut.len() < name.len() { format!("{cut}…") } else { cut }
}

fn issue(code: &'static str, message: String) -> Issue {
    Issue { code, message }
}

fn tag_issue(why: TagIssue, name: &str) -> Issue {
    let name = shown(name);
    match why {
        TagIssue::Empty => issue("empty", format!("\"{name}\" has no letter or digit to make an element name from")),
        TagIssue::LeadingDigit => {
            issue("leadingDigit", format!("\"{name}\" gives an element name that starts with a digit: add a prefix or rename it"))
        }
        TagIssue::Reserved => issue("reserved", format!("\"{name}\" gives an element name the HTML specification reserves: rename it")),
        TagIssue::NoHyphen => issue("noHyphen", format!("\"{name}\" gives an element name without a hyphen: add a prefix ending in one")),
        TagIssue::TooLong => issue("tooLong", format!("\"{name}\" gives an element name of more than 64 characters: shorten it")),
    }
}

fn entry_id(doc: &Document, id: NodeId) -> Option<NodeId> {
    doc.node(id).filter(|n| matches!(n.kind, NodeKind::Component { .. } | NodeKind::ComponentSet { .. }))?;
    Some(variant_set(doc, id).unwrap_or(id))
}

fn candidate(doc: &Document, id: NodeId, needed_by: Option<(String, String)>) -> Option<Candidate> {
    let comp = describe(doc, main_of(doc, id)?)?;
    let name = doc.node(id)?.name.clone();
    let empty = doc.node(comp.comp).is_none_or(|n| n.children.is_empty());
    Some(Candidate { id, name, comp, empty, needed_by })
}

fn all_entries(doc: &Document) -> Vec<NodeId> {
    doc.nodes()
        .filter(|n| match n.kind {
            NodeKind::Component { .. } => variant_set(doc, n.id).is_none(),
            NodeKind::ComponentSet { .. } => true,
            _ => false,
        })
        .map(|n| n.id)
        .collect()
}

/// The components `c` draws as instances, and the ones its instance swaps default to.
fn used_by(doc: &Document, c: &Component) -> Vec<NodeId> {
    let mut stack = c.set.and_then(|s| doc.node(s)).map_or_else(|| vec![c.comp], |s| s.children.clone());
    let mut out: Vec<NodeId> = c.params.iter().filter(|p| p.kind == ParamKind::InstanceSwap).filter_map(|p| id_of(&p.default)).collect();
    let mut seen = HashSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) || seen.len() > MAX_VISITED {
            continue;
        }
        let Some(n) = doc.node(id) else { continue };
        if let NodeKind::Instance { component, .. } = &n.kind {
            out.push(*component);
        }
        stack.extend(n.children.iter().rev().copied());
    }
    out.into_iter().filter_map(|id| entry_id(doc, id)).collect()
}

fn candidates(doc: &Document, ids: Option<&[NodeId]>) -> Vec<Candidate> {
    let roots: Vec<NodeId> = match ids {
        Some(ids) => ids.iter().filter_map(|id| entry_id(doc, *id)).collect(),
        None => all_entries(doc),
    };
    let mut found: Vec<Candidate> = Vec::new();
    for id in roots {
        if !found.iter().any(|c| c.id == id)
            && let Some(c) = candidate(doc, id, None)
        {
            found.push(c);
        }
    }
    let mut next = 0;
    while let Some(c) = found.get(next) {
        let (name, class) = (c.name.clone(), c.comp.type_name.clone());
        for id in used_by(doc, &c.comp) {
            if !found.iter().any(|f| f.id == id)
                && let Some(d) = candidate(doc, id, Some((name.clone(), class.clone())))
            {
                found.push(d);
            }
        }
        next += 1;
    }
    found
}

/// The plan for `opts`: an entry per component, and every file of the package.
pub fn plan(doc: &Document, opts: &ExportOptions) -> ExportPlan {
    let found = candidates(doc, opts.ids.as_deref());
    let tags: Vec<Result<String, TagIssue>> = found.iter().map(|c| lit::tag_name(&c.name, &opts.lit.prefix)).collect();
    let mut taken: BTreeMap<&str, usize> = BTreeMap::new();
    for (c, tag) in found.iter().zip(&tags) {
        if let (false, Ok(tag)) = (c.empty, tag) {
            *taken.entry(tag.as_str()).or_default() += 1;
        }
    }
    let (mut components, mut notes, mut sources) = (Vec::new(), Vec::new(), Vec::new());
    let mut reads: Vec<(String, Vec<String>)> = Vec::new();
    for (c, tag) in found.iter().zip(&tags) {
        let mut issues = Vec::new();
        match tag {
            Err(why) if !c.empty => issues.push(tag_issue(*why, &c.name)),
            Ok(tag) if !c.empty && taken.get(tag.as_str()).is_some_and(|n| *n > 1) => issues.push(issue(
                "duplicate",
                format!("\"{}\" gives the element name <{tag}>, which another component has too: rename one of them", shown(&c.name)),
            )),
            _ => {}
        }
        let status = match (issues.is_empty(), c.empty, &c.needed_by) {
            (false, ..) => Status::Invalid,
            (_, true, _) => Status::Empty,
            (_, _, Some(_)) => Status::Dependency,
            _ => Status::Ok,
        };
        let tag = tag.as_ref().ok().cloned();
        let mut file = None;
        if let (Status::Ok | Status::Dependency, Some(tag)) = (status, &tag) {
            let out = lit::definition(doc, &c.comp, &opts.lit);
            file = Some(format!("components/{tag}.ts"));
            sources.push((tag.clone(), out.class_name, out.ts));
            reads.push((c.comp.type_name.clone(), out.tokens));
            notes.extend(out.notes);
            if let Some((by, by_class)) = &c.needed_by {
                notes.push(Note {
                    code: "dependency-added",
                    component: by_class.clone(),
                    layer: None,
                    message: format!("{} is exported too: {} uses it", shown(&c.name), shown(by)),
                });
            }
        }
        let class_name = tag.as_deref().map(lit::class_of);
        let states = lit::state_recipes(doc, &c.comp);
        components.push(PlannedComponent { id: c.id, name: c.name.clone(), tag, class_name, file, status, issues, states });
    }
    sources.sort_by(|a, b| a.0.cmp(&b.0));

    let selected: Vec<&Collection> =
        doc.variables.collections.iter().filter(|c| opts.collections.as_ref().is_none_or(|ids| ids.contains(&c.id))).collect();
    let tokens =
        tokens_css(doc, &selected, &TokenOptions { prefix: opts.lit.token_prefix.clone(), light: opts.light.clone(), dark: opts.dark.clone() });
    for (component, used) in &reads {
        for name in used.iter().filter(|n| !tokens.declared.contains(n)) {
            let note = Note {
                code: "token-missing",
                component: component.clone(),
                layer: None,
                message: format!("{name} is read by {component} but tokens.css does not declare it"),
            };
            if !notes.contains(&note) {
                notes.push(note);
            }
        }
    }
    notes.extend(tokens.notes);

    let tags: Vec<&str> = sources.iter().map(|(tag, ..)| tag.as_str()).collect();
    let elements: Vec<(&str, &str)> = sources.iter().map(|(tag, class, _)| (tag.as_str(), class.as_str())).collect();
    let mut files: Vec<PlannedFile> =
        sources.iter().map(|(tag, _, ts)| PlannedFile { path: format!("components/{tag}.ts"), kind: "component", text: ts.clone() }).collect();
    let extra = [
        ("tokens/tokens.css", "tokens", tokens.text.clone()),
        ("index.ts", "index", package::index(&tags)),
        ("package.json", "package", package::manifest(&opts.package_name, &opts.version)),
        ("tsconfig.json", "tsconfig", package::tsconfig()),
        ("README.md", "readme", package::readme(&opts.package_name, &elements)),
    ];
    files.extend(extra.into_iter().map(|(path, kind, text)| PlannedFile { path: path.to_string(), kind, text }));
    files.sort_by(|a, b| a.path.cmp(&b.path));
    ExportPlan { components, files, notes }
}
