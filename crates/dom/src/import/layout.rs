//! A flex container becomes an auto layout frame when the layout engine puts its children back on
//! the boxes the browser gave them (1 px tolerance); otherwise the children stay where they were
//! captured and the frame carries a note naming what could not be translated.

use std::collections::BTreeMap;

use figcraft_color::Color;
use figcraft_doc::geom::Affine;
use figcraft_doc::{AutoLayout, CrossAlign, Document, HConstraint, LayoutMode, MainAlign, Node, NodeId, Sizing, VConstraint};
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
        match flex_layout(doc, id, element, sources) {
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
    constrain(doc, root, sources);
}

/// What the layout does not place follows the frame when it is resized, by the edges it was drawn against.
fn constrain(doc: &mut Document, root: NodeId, sources: &BTreeMap<NodeId, &Element>) {
    for id in doc.subtree(root).into_iter().filter(|id| sources.contains_key(id)) {
        let Some(frame) = doc.node(id) else { continue };
        let (width, height, laid_out) = (frame.width, frame.height, frame.layout.is_some());
        for child in frame.children.clone() {
            let Some(node) = doc.node_mut(child) else { continue };
            if laid_out && !node.child_layout.absolute {
                continue;
            }
            let [.., x, y] = node.transform.as_coeffs();
            node.constraints.horizontal = match anchor(x, width - x - node.width) {
                Anchor::Both => HConstraint::LeftRight,
                Anchor::Middle => HConstraint::Center,
                Anchor::End => HConstraint::Right,
                Anchor::Start => HConstraint::Left,
            };
            node.constraints.vertical = match anchor(y, height - y - node.height) {
                Anchor::Both => VConstraint::TopBottom,
                Anchor::Middle => VConstraint::Center,
                Anchor::End => VConstraint::Bottom,
                Anchor::Start => VConstraint::Top,
            };
        }
    }
}

enum Anchor {
    Start,
    Middle,
    End,
    Both,
}

fn anchor(before: f64, after: f64) -> Anchor {
    if before.abs() <= TOLERANCE && after.abs() <= TOLERANCE {
        Anchor::Both
    } else if (before - after).abs() <= TOLERANCE {
        Anchor::Middle
    } else if after < before {
        Anchor::End
    } else {
        Anchor::Start
    }
}

fn report_on(doc: &mut Document, id: NodeId, warnings: &mut Warnings, write: impl FnOnce(&mut Report)) {
    let name = doc.node(id).map(|node| node.name.clone()).unwrap_or_default();
    write(&mut Report::new(&name, warnings));
    notes::attach(doc, id, warnings.take_fresh());
}

fn flex_layout(doc: &Document, id: NodeId, element: &Element, sources: &BTreeMap<NodeId, &Element>) -> Flex {
    let style = &element.style;
    match keyword(style, "display").as_deref() {
        Some("flex" | "inline-flex") => match translate(style) {
            Ok(layout) => Flex::Layout(layout),
            Err(refusal) => Flex::Refused(refusal),
        },
        Some("grid" | "inline-grid") => match grid(doc, id, style, sources) {
            Ok(layout) => Flex::Layout(layout),
            Err(refusal) => Flex::Refused(refusal),
        },
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
    let padding = insets(style, em_px)?;
    let (gap, cross_gap) = if mode == LayoutMode::Vertical { (row_gap, column_gap) } else { (column_gap, row_gap) };
    Ok(AutoLayout { mode, gap, cross_gap, padding, main_align, cross_align, ..AutoLayout::default() })
}

/// A stroke already takes its room in the layout; a border that became no stroke takes it as padding, as it does in a CSS box.
fn insets(style: &Style, em_px: f64) -> Result<[f64; 4], Refusal> {
    let mut insets = [0.0; 4];
    for ((slot, key), side) in insets.iter_mut().zip(PADDING).zip(["Top", "Right", "Bottom", "Left"]) {
        *slot = px(style, key, em_px, false)? + undrawn_border(style, side, em_px);
    }
    Ok(insets)
}

fn undrawn_border(style: &Style, side: &str, em_px: f64) -> f64 {
    let line = keyword(style, &format!("border{side}Style"));
    let visible = get(style, &format!("border{side}Color")).and_then(Color::parse_css).is_some_and(|color| color.a > 0.0);
    match line.as_deref() {
        None | Some("none" | "hidden") => 0.0,
        Some("solid") if visible => 0.0,
        Some(_) => get(style, &format!("border{side}Width")).and_then(|value| length(value, em_px, None)).unwrap_or(0.0).max(0.0),
    }
}

/// A grid that is one row or one column is an auto layout; its alignments are read off the boxes, since its children may each align themselves.
fn grid(doc: &Document, id: NodeId, style: &Style, sources: &BTreeMap<NodeId, &Element>) -> Result<AutoLayout, Refusal> {
    let frame = doc.node(id).ok_or_else(|| refusal(style, "display"))?;
    let in_flow = frame.children.iter().filter(|child| !sources.get(child).is_some_and(|element| is_out_of_flow(&element.style)));
    let boxes: Vec<[f64; 4]> = in_flow.filter_map(|child| doc.node(*child)).map(box_of).collect();
    let same = |at: usize| boxes.iter().all(|one| boxes.first().is_some_and(|first| (one[at] - first[at]).abs() <= TOLERANCE));
    let mode = if same(0) {
        LayoutMode::Vertical
    } else if same(1) {
        LayoutMode::Horizontal
    } else {
        return Err(refusal(style, "display"));
    };
    let em_px = font_size_px(style);
    let padding = insets(style, em_px)?;
    let (column_gap, row_gap) = (px(style, "columnGap", em_px, true)?, px(style, "rowGap", em_px, true)?);
    let [top, right, bottom, left] = padding;
    let (main, gap, cross, cross_gap, main_edges, cross_edges) = if mode == LayoutMode::Horizontal {
        (Axis::X, column_gap, Axis::Y, row_gap, (frame.width, left, right), (frame.height, top, bottom))
    } else {
        (Axis::Y, row_gap, Axis::X, column_gap, (frame.height, top, bottom), (frame.width, left, right))
    };
    let main_align = match placed(&boxes, main, main_edges) {
        Anchor::Middle => MainAlign::Center,
        Anchor::End => MainAlign::Max,
        Anchor::Start | Anchor::Both => MainAlign::Min,
    };
    let cross_align = match placed(&boxes, cross, cross_edges) {
        Anchor::Middle => CrossAlign::Center,
        Anchor::End => CrossAlign::Max,
        Anchor::Start | Anchor::Both => CrossAlign::Min,
    };
    Ok(AutoLayout { mode, gap, cross_gap, padding, main_align, cross_align, ..AutoLayout::default() })
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

fn box_of(node: &Node) -> [f64; 4] {
    let [.., x, y] = node.transform.as_coeffs();
    [x, y, node.width, node.height]
}

fn placed(boxes: &[[f64; 4]], axis: Axis, (size, inset_before, inset_after): (f64, f64, f64)) -> Anchor {
    let span = |[x, y, width, height]: [f64; 4]| match axis {
        Axis::X => (x, x + width),
        Axis::Y => (y, y + height),
    };
    let start = boxes.iter().map(|one| span(*one).0).fold(f64::INFINITY, f64::min);
    let end = boxes.iter().map(|one| span(*one).1).fold(f64::NEG_INFINITY, f64::max);
    anchor(start - inset_before, size - inset_after - end)
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
    fill_children(doc, id, &layout, sources);
    if let Some(node) = doc.node_mut(id) {
        node.layout = Some(layout);
    }
    true
}

/// A child the browser stretches or grows fills its frame, so that it follows when the frame is resized. A text run keeps its own box: its captured width is the ink's, not the track's.
fn fill_children(doc: &mut Document, id: NodeId, layout: &AutoLayout, sources: &BTreeMap<NodeId, &Element>) {
    let Some(frame) = doc.node(id) else { return };
    let vertical = layout.mode == LayoutMode::Vertical;
    let [top, right, bottom, left] = layout.padding;
    let inner_cross = if vertical { frame.width - left - right } else { frame.height - top - bottom };
    let stretches =
        sources.get(&id).is_some_and(|element| matches!(keyword(&element.style, "alignItems").as_deref(), None | Some("stretch" | "normal")));
    let in_flow: Vec<NodeId> =
        frame.children.iter().copied().filter(|child| sources.get(child).is_some_and(|element| !is_out_of_flow(&element.style))).collect();
    let grows = |child: &NodeId| {
        sources
            .get(child)
            .and_then(|element| get(&element.style, "flexGrow"))
            .and_then(|value| value.parse::<f64>().ok())
            .is_some_and(|grow| grow > 0.0)
    };
    let lone_grower = in_flow.iter().filter(|child| grows(child)).count() == 1;
    for child in in_flow.iter().copied() {
        let self_align = sources.get(&child).and_then(|element| keyword(&element.style, "alignSelf"));
        let stretched = match self_align.as_deref() {
            None | Some("auto") => stretches,
            Some(word) => word == "stretch",
        };
        let grown = lone_grower && grows(&child);
        let Some(node) = doc.node_mut(child) else { continue };
        let cross = if vertical { node.width } else { node.height };
        let cross_fill = stretched && (cross - inner_cross).abs() <= TOLERANCE;
        let (horizontal, upright) = if vertical { (cross_fill, grown) } else { (grown, cross_fill) };
        if horizontal {
            node.child_layout.sizing_h = Sizing::Fill;
        }
        if upright {
            node.child_layout.sizing_v = Sizing::Fill;
        }
    }
}

fn is_out_of_flow(style: &Style) -> bool {
    matches!(keyword(style, "position").as_deref(), Some("absolute" | "fixed"))
}

fn clear_layout(doc: &mut Document, id: NodeId) {
    let children = doc.node(id).map(|node| node.children.clone()).unwrap_or_default();
    for child in children {
        if let Some(node) = doc.node_mut(child) {
            node.child_layout.absolute = false;
            node.child_layout.sizing_h = Sizing::Fixed;
            node.child_layout.sizing_v = Sizing::Fixed;
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
