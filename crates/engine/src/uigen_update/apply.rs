//! Taking differences into the copy: a layer added or removed, or one group of properties copied from
//! the same layer of the latest capture. A difference that can't be taken is skipped with its reason.

use figcraft_doc::{Document, Node, NodeId, NodeKind};

use super::diff::{Difference, DifferenceKind, Field, layers, parent_path, variant_ids};
use crate::REFERENCE_KEY;

const MISSING_VARIANT: &str = "the variant is no longer in your copy";
const MISSING_IN_LATEST: &str = "the layer is not in the new capture";
const MISSING_IN_COPY: &str = "the layer is no longer in your copy";
const NOT_APPLICABLE: &str = "this kind of change can't be applied automatically";

#[derive(Debug, Default)]
pub(crate) struct Outcome {
    pub applied: Vec<String>,
    pub skipped: Vec<(String, String)>,
}

pub(crate) fn apply(doc: &mut Document, copy: NodeId, latest: NodeId, chosen: &[Difference]) -> Outcome {
    let mut outcome = Outcome::default();
    for difference in chosen {
        match apply_one(doc, copy, latest, difference) {
            Ok(()) => outcome.applied.push(difference.id.clone()),
            Err(reason) => outcome.skipped.push((difference.id.clone(), reason)),
        }
    }
    outcome
}

/// The originals of the variants of `copy` become those of the same-name variants of `latest`.
pub(crate) fn follow_references(doc: &mut Document, copy: NodeId, latest: NodeId) -> crate::Result<()> {
    for (name, variant) in variant_ids(doc, copy) {
        let original = variant_named(doc, latest, &name).and_then(|id| doc.node(id)).and_then(|node| node.plugin_data.get(REFERENCE_KEY)).cloned();
        let data = &mut doc.get_mut(variant)?.plugin_data;
        match original {
            Some(raw) => data.insert(REFERENCE_KEY.to_owned(), raw),
            None => data.remove(REFERENCE_KEY),
        };
    }
    Ok(())
}

fn variant_named(doc: &Document, set: NodeId, name: &str) -> Option<NodeId> {
    variant_ids(doc, set).into_iter().find(|(variant, _)| variant == name).map(|(_, id)| id)
}

fn apply_one(doc: &mut Document, copy: NodeId, latest: NodeId, difference: &Difference) -> Result<(), String> {
    let copy_variant = variant_named(doc, copy, &difference.variant).ok_or(MISSING_VARIANT)?;
    let latest_variant = variant_named(doc, latest, &difference.variant).ok_or("the variant is not in the new capture")?;
    let (in_copy, in_latest) = (layers(doc, copy_variant), layers(doc, latest_variant));
    let path = difference.path.as_str();
    match difference.kind {
        DifferenceKind::Added => {
            let source = *in_latest.by_path.get(path).ok_or(MISSING_IN_LATEST)?;
            let parent = match parent_path(path) {
                None => copy_variant,
                Some(parent) => *in_copy.by_path.get(parent).ok_or_else(|| format!("its parent \"{parent}\" is no longer in your copy"))?,
            };
            let place = place_of(doc, source);
            let index = place.min(doc.node(parent).map_or(0, |node| node.children.len()));
            doc.duplicate_into(source, parent, Some(index)).map(|_| ()).map_err(|e| e.to_string())
        }
        DifferenceKind::Removed => {
            let target = *in_copy.by_path.get(path).ok_or("the layer is already gone from your copy")?;
            doc.remove(target).map_err(|e| e.to_string())
        }
        DifferenceKind::Changed => {
            let on_root = path.is_empty();
            let field = Field::from_name(&difference.field)
                .filter(|field| *field != Field::Other && !(on_root && *field == Field::Position))
                .ok_or(NOT_APPLICABLE)?;
            let target = *in_copy.by_path.get(path).ok_or(MISSING_IN_COPY)?;
            let source = in_latest.by_path.get(path).and_then(|id| doc.node(*id)).cloned().ok_or(MISSING_IN_LATEST)?;
            copy_field(field, doc.get_mut(target).map_err(|e| e.to_string())?, &source);
            Ok(())
        }
    }
}

fn place_of(doc: &Document, layer: NodeId) -> usize {
    doc.node(layer)
        .and_then(|node| node.parent)
        .and_then(|parent| doc.node(parent))
        .and_then(|parent| parent.children.iter().position(|child| *child == layer))
        .unwrap_or(usize::MAX)
}

fn follow(to: &mut Node, from: &Node, bound: impl Fn(&str) -> bool) {
    to.bound.retain(|key, _| !bound(key));
    to.bound.extend(from.bound.iter().filter(|(key, _)| bound(key)).map(|(key, variable)| (key.clone(), variable.clone())));
}

fn copy_field(field: Field, to: &mut Node, from: &Node) {
    match field {
        Field::Fill => {
            to.fills.clone_from(&from.fills);
            to.styles.fill.clone_from(&from.styles.fill);
            follow(to, from, |key| key == "fill" || key.starts_with("fills"));
        }
        Field::Stroke => {
            to.strokes.clone_from(&from.strokes);
            to.stroke = from.stroke.clone();
            to.styles.stroke.clone_from(&from.styles.stroke);
            follow(to, from, |key| key == "stroke" || key == "strokeWeight" || key.starts_with("strokes"));
        }
        Field::Effects => {
            to.effects.clone_from(&from.effects);
            to.styles.effect.clone_from(&from.styles.effect);
            follow(to, from, |key| key.starts_with("effects"));
        }
        Field::CornerRadius => {
            to.corners = from.corners;
            follow(to, from, |key| key.to_ascii_lowercase().contains("radius"));
        }
        Field::Size => {
            (to.width, to.height) = (from.width, from.height);
            follow(to, from, |key| key == "width" || key == "height");
        }
        Field::Position => to.transform = from.transform,
        Field::Opacity => {
            to.opacity = from.opacity;
            follow(to, from, |key| key == "opacity");
        }
        Field::Blend => to.blend = from.blend,
        Field::Visibility => to.visible = from.visible,
        Field::ClipContent => to.clip = from.clip,
        Field::Text => {
            if let (NodeKind::Text(into), NodeKind::Text(source)) = (&mut to.kind, &from.kind) {
                into.characters.clone_from(&source.characters);
                into.runs.clone_from(&source.runs);
            }
            follow(to, from, |key| key == "characters");
        }
        Field::Font => {
            if let (NodeKind::Text(into), NodeKind::Text(source)) = (&mut to.kind, &from.kind) {
                into.style.clone_from(&source.style);
                into.align_h = source.align_h;
                into.align_v = source.align_v;
                into.paragraph_spacing = source.paragraph_spacing;
                into.paragraph_indent = source.paragraph_indent;
                into.list_spacing = source.list_spacing;
                into.leading_trim = source.leading_trim;
                into.baseline_nudge = source.baseline_nudge;
                into.auto_resize = source.auto_resize;
            }
            to.styles.text.clone_from(&from.styles.text);
            follow(to, from, |key| key.starts_with("font") || key == "lineHeight" || key == "letterSpacing");
        }
        Field::AutoLayout => {
            to.layout.clone_from(&from.layout);
            follow(to, from, |key| key == "gap" || key == "crossGap" || key.starts_with("padding"));
        }
        Field::Other => {}
    }
}
