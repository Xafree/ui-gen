//! Layers paint in list order, CSS in stacking order: negative `z-index`, then the normal flow, then
//! positioned elements. Only children that really overlap are moved, an auto layout reads the list as its flow.

use super::css::{get, keyword};
use crate::model::{DomBox, DomNode};

#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct Rank {
    level: u8,
    z: i32,
}

fn rank_of(node: &DomNode) -> Rank {
    let DomNode::Element(element) = node else { return Rank { level: 1, z: 0 } };
    let positioned = keyword(&element.style, "position").is_some_and(|position| position != "static");
    let z = get(&element.style, "zIndex").and_then(|value| value.parse::<i32>().ok()).filter(|_| positioned).unwrap_or(0);
    let level = match (positioned, z) {
        (true, ..0) => 0,
        (false, _) => 1,
        (true, 0) => 2,
        (true, _) => 3,
    };
    Rank { level, z }
}

fn box_of(node: &DomNode) -> &DomBox {
    match node {
        DomNode::Element(element) => &element.r#box,
        DomNode::Text(run) => &run.r#box,
        DomNode::Svg(svg) => &svg.r#box,
    }
}

fn overlap(first: &DomBox, second: &DomBox) -> bool {
    first.x < second.x + second.width && second.x < first.x + first.width && first.y < second.y + second.height && second.y < first.y + first.height
}

pub(super) fn painting_order(children: &[DomNode]) -> Vec<&DomNode> {
    let ranked: Vec<(Rank, &DomNode)> = children.iter().map(|child| (rank_of(child), child)).collect();
    let misplaced = ranked
        .iter()
        .enumerate()
        .any(|(at, (rank, child))| ranked.iter().skip(at + 1).any(|(later, other)| later < rank && overlap(box_of(child), box_of(other))));
    if !misplaced {
        return children.iter().collect();
    }
    let mut sorted = ranked;
    sorted.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(std::cmp::Ordering::Equal));
    sorted.into_iter().map(|(_, child)| child).collect()
}
