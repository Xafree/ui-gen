//! What changed between the capture a copy comes from (the base) and the current one (the latest), and
//! where the copy stands against it. Variants match by name, layers by their path of names.

use std::collections::{BTreeMap, BTreeSet};

use figcraft_doc::{Document, NodeId, NodeKind};

use crate::devcompare::{PropChange, design_changes};
use crate::uigen_import::{is_import_page, page_of, set_of, source_ref};
use crate::{Result, bad};

const COMMAND: &str = "uigen.sourceDiff";
const SEPARATOR: &str = " / ";
const LAYER: &str = "layer";

pub const MAX_DIFFERENCES: usize = 2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DifferenceKind {
    Added,
    Removed,
    Changed,
}

impl DifferenceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Changed => "changed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DifferenceState {
    Clean,
    Modified,
    Unknown,
}

impl DifferenceState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Modified => "modified",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Difference {
    pub id: String,
    pub variant: String,
    pub path: String,
    pub kind: DifferenceKind,
    pub field: String,
    pub state: DifferenceState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Located {
    pub page: NodeId,
    pub set: NodeId,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub clean: usize,
    pub modified: usize,
    pub unknown: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SourceDiff {
    pub copy: NodeId,
    pub name: String,
    pub from: String,
    pub to: String,
    pub base: Option<Located>,
    pub latest: Option<Located>,
    pub differences: Vec<Difference>,
    pub summary: Summary,
    pub truncated: bool,
    pub note: Option<String>,
}

/// The groups of properties a difference is told and applied by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Field {
    Fill,
    Stroke,
    Effects,
    CornerRadius,
    Size,
    Position,
    Opacity,
    Blend,
    Visibility,
    ClipContent,
    Text,
    Font,
    AutoLayout,
    Other,
}

impl Field {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Fill => "fill",
            Self::Stroke => "stroke",
            Self::Effects => "effects",
            Self::CornerRadius => "corner radius",
            Self::Size => "size",
            Self::Position => "position",
            Self::Opacity => "opacity",
            Self::Blend => "blend",
            Self::Visibility => "visibility",
            Self::ClipContent => "clip content",
            Self::Text => "text",
            Self::Font => "font",
            Self::AutoLayout => "auto layout",
            Self::Other => "other",
        }
    }

    pub(super) fn from_name(name: &str) -> Option<Self> {
        const ALL: [Field; 14] = [
            Field::Fill,
            Field::Stroke,
            Field::Effects,
            Field::CornerRadius,
            Field::Size,
            Field::Position,
            Field::Opacity,
            Field::Blend,
            Field::Visibility,
            Field::ClipContent,
            Field::Text,
            Field::Font,
            Field::AutoLayout,
            Field::Other,
        ];
        ALL.into_iter().find(|field| field.name() == name)
    }

    fn of(change: &PropChange) -> Self {
        match (change.section, change.property.as_str()) {
            ("Fills", _) => Self::Fill,
            ("Strokes", _) => Self::Stroke,
            ("Effects", _) => Self::Effects,
            ("Auto layout", _) => Self::AutoLayout,
            ("Typography", _) => Self::Font,
            ("Layout", "width" | "height") => Self::Size,
            ("Layout", "top" | "left" | "rotation") => Self::Position,
            ("Layout", "opacity") => Self::Opacity,
            ("Layout", "corner radius") => Self::CornerRadius,
            (_, "blend") => Self::Blend,
            ("Layer", "visibility") => Self::Visibility,
            ("Layer", "clip content") => Self::ClipContent,
            ("Layer", "text") => Self::Text,
            _ => Self::Other,
        }
    }
}

pub(super) struct Layers {
    pub order: Vec<String>,
    pub by_path: BTreeMap<String, NodeId>,
}

fn escaped(name: &str) -> String {
    name.replace('\\', "\\\\").replace('/', "\\/")
}

pub(super) fn parent_path(path: &str) -> Option<&str> {
    path.rfind(SEPARATOR).map(|at| &path[..at])
}

pub(super) fn variant_ids(doc: &Document, set: NodeId) -> Vec<(String, NodeId)> {
    let Some(node) = doc.node(set) else { return Vec::new() };
    let mut found: Vec<(String, NodeId)> = Vec::new();
    for child in &node.children {
        let Some(variant) = doc.node(*child).filter(|variant| matches!(variant.kind, NodeKind::Component { .. })) else { continue };
        if !found.iter().any(|(name, _)| *name == variant.name) {
            found.push((variant.name.clone(), *child));
        }
    }
    found
}

/// The layers under a variant in tree order, by path. The first of two siblings with one name keeps the path.
pub(super) fn layers(doc: &Document, variant: NodeId) -> Layers {
    let mut found = Layers { order: Vec::new(), by_path: BTreeMap::new() };
    let Some(root) = doc.node(variant) else { return found };
    let mut stack: Vec<(NodeId, Option<String>)> = root.children.iter().rev().map(|child| (*child, None)).collect();
    while let Some((id, parent)) = stack.pop() {
        let Some(node) = doc.node(id) else { continue };
        let name = escaped(&node.name);
        let path = parent.map_or_else(|| name.clone(), |parent| format!("{parent}{SEPARATOR}{name}"));
        if found.by_path.contains_key(&path) {
            continue;
        }
        found.by_path.insert(path.clone(), id);
        found.order.push(path.clone());
        stack.extend(node.children.iter().rev().map(|child| (*child, Some(path.clone()))));
    }
    found
}

struct Tree {
    variants: Vec<String>,
    layers: BTreeMap<String, Layers>,
}

impl Tree {
    fn of(doc: &Document, set: NodeId) -> Self {
        let ids = variant_ids(doc, set);
        Self {
            variants: ids.iter().map(|(name, _)| name.clone()).collect(),
            layers: ids.iter().map(|(name, id)| (name.clone(), layers(doc, *id))).collect(),
        }
    }

    fn at(&self, variant: &str, path: &str) -> Option<NodeId> {
        self.layers.get(variant)?.by_path.get(path).copied()
    }

    fn can_hold(&self, variant: &str, path: &str) -> bool {
        self.layers.get(variant).is_some_and(|layers| holds_parent(layers, path))
    }
}

fn holds_parent(layers: &Layers, path: &str) -> bool {
    parent_path(path).is_none_or(|parent| layers.by_path.contains_key(parent))
}

fn fields_between(doc: &Document, from: NodeId, to: NodeId) -> BTreeSet<Field> {
    design_changes(doc, from, doc, to).iter().map(Field::of).collect()
}

struct Raw {
    variant: String,
    path: String,
    kind: DifferenceKind,
    field: &'static str,
}

/// What must change in `from` to become `to`, a layer being listed once, at the top of what was added or removed.
fn raw_diff(doc: &Document, from: &Tree, to: &Tree) -> Vec<Raw> {
    let mut found = Vec::new();
    for variant in &to.variants {
        let (Some(before), Some(after)) = (from.layers.get(variant), to.layers.get(variant)) else { continue };
        let raw = |path: &String, kind, field| Raw { variant: variant.clone(), path: path.clone(), kind, field };
        for path in &after.order {
            match (before.by_path.get(path), after.by_path.get(path)) {
                (None, _) if holds_parent(before, path) => found.push(raw(path, DifferenceKind::Added, LAYER)),
                (Some(was), Some(now)) => {
                    found.extend(fields_between(doc, *was, *now).into_iter().map(|field| raw(path, DifferenceKind::Changed, field.name())));
                }
                _ => {}
            }
        }
        for path in before.order.iter().filter(|path| !after.by_path.contains_key(*path) && holds_parent(after, path)) {
            found.push(raw(path, DifferenceKind::Removed, LAYER));
        }
    }
    found
}

struct Captures<'a> {
    base: &'a Tree,
    copy: &'a Tree,
    latest: &'a Tree,
}

/// Where the copy stands on a difference between base and latest: `None` when it already has the new value.
fn state_of(doc: &Document, raw: &Raw, captures: &Captures) -> Option<DifferenceState> {
    let (variant, path) = (raw.variant.as_str(), raw.path.as_str());
    let (base, copy) = (captures.base.at(variant, path), captures.copy.at(variant, path));
    match raw.kind {
        DifferenceKind::Added if copy.is_some() => None,
        DifferenceKind::Added if !captures.copy.can_hold(variant, path) => Some(DifferenceState::Modified),
        DifferenceKind::Added => Some(DifferenceState::Clean),
        DifferenceKind::Removed => match (copy, base) {
            (None, _) => None,
            (Some(copy), Some(base)) if fields_between(doc, base, copy).is_empty() => Some(DifferenceState::Clean),
            _ => Some(DifferenceState::Modified),
        },
        DifferenceKind::Changed => {
            let (base, latest) = (base?, captures.latest.at(variant, path)?);
            let Some(copy) = copy else { return Some(DifferenceState::Modified) };
            let field = Field::from_name(raw.field)?;
            if !fields_between(doc, copy, latest).contains(&field) {
                return None;
            }
            Some(if fields_between(doc, base, copy).contains(&field) { DifferenceState::Modified } else { DifferenceState::Clean })
        }
    }
}

fn difference_of(raw: Raw, state: DifferenceState) -> Difference {
    Difference {
        id: format!("{}|{}|{}", raw.variant, raw.path, raw.field),
        variant: raw.variant,
        path: raw.path,
        kind: raw.kind,
        field: raw.field.to_owned(),
        state,
    }
}

fn located(doc: &Document, package: &str, version: &str, component: &str) -> Option<Located> {
    let page = page_of(doc, package, version)?;
    Some(Located { page, set: set_of(doc, page, component)? })
}

fn listed(doc: &Document, copy: NodeId, base: Option<Located>, latest: Located) -> Vec<Difference> {
    let (copy_tree, latest_tree) = (Tree::of(doc, copy), Tree::of(doc, latest.set));
    match base {
        Some(base) if base.set == latest.set => Vec::new(),
        Some(base) => {
            let base_tree = Tree::of(doc, base.set);
            let captures = Captures { base: &base_tree, copy: &copy_tree, latest: &latest_tree };
            raw_diff(doc, &base_tree, &latest_tree).into_iter().filter_map(|raw| difference_with_state(doc, raw, &captures)).collect()
        }
        None => raw_diff(doc, &copy_tree, &latest_tree).into_iter().map(|raw| difference_of(raw, DifferenceState::Unknown)).collect(),
    }
}

fn difference_with_state(doc: &Document, raw: Raw, captures: &Captures) -> Option<Difference> {
    let state = state_of(doc, &raw, captures)?;
    Some(difference_of(raw, state))
}

fn summary_of(differences: &[Difference]) -> Summary {
    let count = |state| differences.iter().filter(|difference| difference.state == state).count();
    Summary { clean: count(DifferenceState::Clean), modified: count(DifferenceState::Modified), unknown: count(DifferenceState::Unknown) }
}

/// The differences between the capture `copy` comes from and the one of `current_version`, each told
/// by where the copy stands: `Clean` when it still has the old value, `Modified` when the user changed it,
/// `Unknown` when the old capture is gone. A difference the copy already has the new value of is not listed.
pub fn source_diff(doc: &Document, copy: NodeId, current_version: &str) -> Result<SourceDiff> {
    let node = doc.node(copy).ok_or_else(|| bad(COMMAND, format!("`copy` {copy} does not exist")))?;
    if !matches!(node.kind, NodeKind::ComponentSet { .. }) {
        return Err(bad(
            COMMAND,
            format!("`copy` {copy} is a {}: only the copy of an imported component set has differences", node.kind.type_name()),
        ));
    }
    if node.parent.is_some_and(|parent| is_import_page(doc, parent)) {
        return Err(bad(COMMAND, format!("`copy` {copy} is in an import page: choose the copy placed in the library")));
    }
    let origin = source_ref(doc, copy)
        .ok_or_else(|| bad(COMMAND, format!("`copy` {copy} has no readable uigen.source: it is not the copy of an imported component")))?;
    let base = located(doc, &origin.package, &origin.version, &origin.component);
    let latest = located(doc, &origin.package, current_version, &origin.component);
    let mut note = None;
    let mut differences = match latest {
        Some(latest) => listed(doc, copy, base, latest),
        None => {
            note = Some(format!("recapture first: {current_version} has not been captured yet"));
            Vec::new()
        }
    };
    let truncated = differences.len() > MAX_DIFFERENCES;
    if truncated {
        differences.truncate(MAX_DIFFERENCES);
        note = Some(format!("more than {MAX_DIFFERENCES} differences: the first {MAX_DIFFERENCES} are listed"));
    }
    let summary = summary_of(&differences);
    Ok(SourceDiff {
        copy,
        name: node.name.clone(),
        from: origin.version,
        to: current_version.to_owned(),
        base,
        latest,
        differences,
        summary,
        truncated,
        note,
    })
}
