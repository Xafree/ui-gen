//! A flex container becomes an auto layout frame when the layout engine puts its children back on
//! the boxes the browser gave them (1 px tolerance); otherwise the children stay where they were
//! captured and the frame carries a note naming what could not be translated.

use std::collections::BTreeMap;

use figcraft_doc::geom::Affine;
use figcraft_doc::{AutoLayout, CrossAlign, Document, LayoutMode, MainAlign, NodeId};
use figcraft_layout::NoMeasure;

use super::css::{Style, font_size_px, get, keyword, length};
use super::{Report, Warnings, notes};
use crate::model::Element;

const TOLERANCE: f64 = 1.0;
const PADDING: [&str; 4] = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];

struct Refusal {
    property: &'static str,
    value: String,
}

enum Flex {
    Not,
    Refused(Refusal),
    Layout(AutoLayout),
}

type Captured = BTreeMap<NodeId, (Affine, f64, f64)>;

pub(super) fn apply(doc: &mut Document, root: NodeId, sources: &BTreeMap<NodeId, &Element>, warnings: &mut Warnings) {
    let captured = capture(doc, root);
    let mut tentative = Vec::new();
    for id in doc.subtree(root) {
        let Some(element) = sources.get(&id) else { continue };
        match flex_layout(element) {
            Flex::Not => {}
            Flex::Refused(refusal) => report_on(doc, id, warnings, |report| report.unreadable(refusal.property, &refusal.value)),
            Flex::Layout(layout) => {
                if set_layout(doc, id, layout, sources) {
                    tentative.push(id);
                }
            }
        }
    }
    figcraft_layout::layout_page(doc, root, &NoMeasure);
    let drifted: Vec<NodeId> = tentative.into_iter().filter(|id| !children_on_captured_boxes(doc, *id, &captured)).collect();
    restore(doc, &captured);
    for id in drifted {
        clear_layout(doc, id);
        report_on(doc, id, warnings, |report| {
            report.note(&format!("display flex: the positions differ by more than {TOLERANCE} px from the captured boxes"))
        });
    }
}

fn report_on(doc: &mut Document, id: NodeId, warnings: &mut Warnings, write: impl FnOnce(&mut Report)) {
    let name = doc.node(id).map(|node| node.name.clone()).unwrap_or_default();
    write(&mut Report::new(&name, warnings));
    notes::attach(doc, id, warnings.take_fresh());
}

fn flex_layout(element: &Element) -> Flex {
    let style = &element.style;
    match keyword(style, "display").as_deref() {
        Some("flex" | "inline-flex") => match translate(style) {
            Ok(layout) => Flex::Layout(layout),
            Err(refusal) => Flex::Refused(refusal),
        },
        Some("grid" | "inline-grid") => Flex::Refused(refusal(style, "display")),
        _ => Flex::Not,
    }
}

fn refusal(style: &Style, property: &'static str) -> Refusal {
    Refusal { property, value: get(style, property).unwrap_or_default().to_owned() }
}

fn translate(style: &Style) -> Result<AutoLayout, Refusal> {
    let mode = choice(style, "flexDirection", &[(&["row"], LayoutMode::Horizontal), (&["column"], LayoutMode::Vertical)], LayoutMode::Horizontal)?;
    if keyword(style, "flexWrap").is_some_and(|wrap| wrap != "nowrap") {
        return Err(refusal(style, "flexWrap"));
    }
    let main_align = choice(
        style,
        "justifyContent",
        &[
            (&["flex-start", "start", "normal", "left"], MainAlign::Min),
            (&["center"], MainAlign::Center),
            (&["flex-end", "end", "right"], MainAlign::Max),
            (&["space-between"], MainAlign::SpaceBetween),
        ],
        MainAlign::Min,
    )?;
    let cross_align = choice(
        style,
        "alignItems",
        &[
            (&["flex-start", "start", "self-start", "stretch", "normal"], CrossAlign::Min),
            (&["center"], CrossAlign::Center),
            (&["flex-end", "end", "self-end"], CrossAlign::Max),
            (&["baseline"], CrossAlign::Baseline),
        ],
        CrossAlign::Min,
    )?;
    let em_px = font_size_px(style);
    let (column_gap, row_gap) = (px(style, "columnGap", em_px, true)?, px(style, "rowGap", em_px, true)?);
    let mut padding = [0.0; 4];
    for (slot, key) in padding.iter_mut().zip(PADDING) {
        *slot = px(style, key, em_px, false)?;
    }
    let (gap, cross_gap) = if mode == LayoutMode::Vertical { (row_gap, column_gap) } else { (column_gap, row_gap) };
    Ok(AutoLayout { mode, gap, cross_gap, padding, main_align, cross_align, ..AutoLayout::default() })
}

fn choice<T: Copy>(style: &Style, key: &'static str, accepted: &[(&[&str], T)], default: T) -> Result<T, Refusal> {
    let Some(word) = keyword(style, key) else { return Ok(default) };
    accepted.iter().find(|(words, _)| words.contains(&word.as_str())).map(|(_, value)| *value).ok_or_else(|| refusal(style, key))
}

fn px(style: &Style, key: &'static str, em_px: f64, normal_is_zero: bool) -> Result<f64, Refusal> {
    let Some(value) = get(style, key) else { return Ok(0.0) };
    if normal_is_zero && value.eq_ignore_ascii_case("normal") {
        return Ok(0.0);
    }
    length(value, em_px, None).filter(|px| *px >= 0.0).ok_or_else(|| refusal(style, key))
}

fn set_layout(doc: &mut Document, id: NodeId, layout: AutoLayout, sources: &BTreeMap<NodeId, &Element>) -> bool {
    let Some(children) = doc.node(id).map(|node| node.children.clone()).filter(|children| !children.is_empty()) else { return false };
    for child in children.into_iter().filter(|child| sources.get(child).is_some_and(|element| is_out_of_flow(&element.style))) {
        if let Some(node) = doc.node_mut(child) {
            node.child_layout.absolute = true;
        }
    }
    if let Some(node) = doc.node_mut(id) {
        node.layout = Some(layout);
    }
    true
}

fn is_out_of_flow(style: &Style) -> bool {
    matches!(keyword(style, "position").as_deref(), Some("absolute" | "fixed"))
}

fn clear_layout(doc: &mut Document, id: NodeId) {
    let children = doc.node(id).map(|node| node.children.clone()).unwrap_or_default();
    for child in children {
        if let Some(node) = doc.node_mut(child) {
            node.child_layout.absolute = false;
        }
    }
    if let Some(node) = doc.node_mut(id) {
        node.layout = None;
    }
}

fn capture(doc: &Document, root: NodeId) -> Captured {
    doc.subtree(root).into_iter().filter_map(|id| doc.node(id).map(|node| (id, (node.transform, node.width, node.height)))).collect()
}

fn restore(doc: &mut Document, captured: &Captured) {
    for (id, (transform, width, height)) in captured {
        if let Some(node) = doc.node_mut(*id) {
            node.transform = *transform;
            node.width = *width;
            node.height = *height;
        }
    }
}

fn children_on_captured_boxes(doc: &Document, id: NodeId, captured: &Captured) -> bool {
    let Some(frame) = doc.node(id) else { return false };
    frame.children.iter().all(|child| {
        let (Some(node), Some((transform, width, height))) = (doc.node(*child), captured.get(child)) else { return false };
        let (now, then) = (node.transform.as_coeffs(), transform.as_coeffs());
        [now[4] - then[4], now[5] - then[5], node.width - width, node.height - height].into_iter().all(|gap| gap.abs() <= TOLERANCE)
    })
}
