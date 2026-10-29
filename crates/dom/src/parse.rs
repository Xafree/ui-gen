use serde::Deserialize;

use crate::error::{SnapshotError, capped};
use crate::model::{DomBox, DomNode, Snapshot};
use crate::{MAX_DEPTH, MAX_JSON_BYTES, MAX_MARKUP_BYTES, MAX_NODES, SUPPORTED_VERSION};

/// serde_json stops at 128 nested levels, a 64-level tree needs 129: the limit is lifted and this bound replaces it.
const MAX_JSON_NESTING: usize = 2 * MAX_DEPTH + 8;

const MAX_TAG_CHARS: usize = 80;

#[derive(Deserialize)]
struct VersionProbe {
    version: u32,
}

pub fn parse_snapshot(json: &str) -> Result<Snapshot, SnapshotError> {
    if json.len() > MAX_JSON_BYTES {
        return Err(SnapshotError::TooLarge { size: json.len() });
    }
    if nesting_exceeds(json, MAX_JSON_NESTING) {
        return Err(SnapshotError::TooDeep);
    }
    let probe: VersionProbe = deserialize(json)?;
    if probe.version != SUPPORTED_VERSION {
        return Err(SnapshotError::UnsupportedVersion { found: probe.version });
    }
    let snapshot: Snapshot = deserialize(json)?;
    validate_tree(&snapshot.root)?;
    Ok(snapshot)
}

fn deserialize<'a, T: Deserialize<'a>>(json: &'a str) -> Result<T, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(json);
    deserializer.disable_recursion_limit();
    let value = T::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

fn nesting_exceeds(json: &str, limit: usize) -> bool {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for &byte in json.as_bytes() {
        if in_string {
            match (escaped, byte) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > limit {
                    return true;
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

fn validate_tree(root: &DomNode) -> Result<(), SnapshotError> {
    let mut pending = vec![(root, 1usize)];
    let mut visited = 0usize;
    while let Some((node, depth)) = pending.pop() {
        visited += 1;
        if visited > MAX_NODES {
            return Err(SnapshotError::TooManyNodes);
        }
        if depth > MAX_DEPTH {
            return Err(SnapshotError::TooDeep);
        }
        validate_node(node)?;
        if let DomNode::Element(element) = node {
            pending.extend(element.children.iter().map(|child| (child, depth + 1)));
        }
    }
    Ok(())
}

fn validate_node(node: &DomNode) -> Result<(), SnapshotError> {
    match node {
        DomNode::Element(element) => validate_box(&element.r#box, || format!("element <{}>", capped(&element.tag, MAX_TAG_CHARS))),
        DomNode::Text(text) => {
            validate_box(&text.r#box, || "text".to_owned())?;
            text.line_box.as_ref().map_or(Ok(()), |line_box| validate_box(line_box, || "text line box".to_owned()))
        }
        DomNode::Svg(svg) => {
            if svg.markup.len() > MAX_MARKUP_BYTES {
                return Err(SnapshotError::MarkupTooLarge { node: "svg".to_owned(), size: svg.markup.len() });
            }
            validate_box(&svg.r#box, || "svg".to_owned())
        }
    }
}

fn validate_box(dom_box: &DomBox, node: impl FnOnce() -> String) -> Result<(), SnapshotError> {
    let valid = dom_box.x.is_finite()
        && dom_box.y.is_finite()
        && dom_box.width.is_finite()
        && dom_box.height.is_finite()
        && dom_box.width >= 0.0
        && dom_box.height >= 0.0;
    if valid { Ok(()) } else { Err(SnapshotError::InvalidBox { node: node() }) }
}
