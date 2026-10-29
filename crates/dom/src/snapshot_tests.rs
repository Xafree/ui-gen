use super::*;

const BOX: &str = r#"{"x":0,"y":0,"width":10,"height":10}"#;

fn snapshot_with_root(root: &str) -> String {
    format!(r#"{{"version":1,"viewport":{{"width":400,"height":200,"deviceScaleFactor":1}},"root":{root}}}"#)
}

fn text_node() -> String {
    format!(r#"{{"type":"text","text":"a","box":{BOX}}}"#)
}

fn error_message(json: &str) -> String {
    match parse_snapshot(json) {
        Ok(_) => panic!("expected the snapshot to be refused"),
        Err(error) => error.to_string().to_lowercase(),
    }
}

fn nested_elements(depth: usize) -> String {
    let leaf = format!(r#"{{"type":"element","tag":"span","box":{BOX}}}"#);
    let mut node = leaf;
    for _ in 1..depth {
        node = format!(r#"{{"type":"element","tag":"div","box":{BOX},"children":[{node}]}}"#);
    }
    snapshot_with_root(&node)
}

fn flat_nodes(total: usize) -> String {
    let children = vec![text_node(); total - 1].join(",");
    snapshot_with_root(&format!(r#"{{"type":"element","tag":"div","box":{BOX},"children":[{children}]}}"#))
}

// covers: D1
#[test]
fn valid_v1_snapshot_reads_element_text_and_svg() {
    let json = r#"{
        "version": 1,
        "source": {"package": "@mui/material", "version": "6.0.0", "component": "Button", "props": {"variant": "contained"}, "state": "hover"},
        "viewport": {"width": 400, "height": 200, "deviceScaleFactor": 2},
        "root": {
            "type": "element", "tag": "button", "label": "Bouton",
            "box": {"x": 20, "y": 20, "width": 82.3, "height": 36.5},
            "style": {"backgroundColor": "rgb(25, 118, 210)"},
            "children": [
                {"type": "text", "text": "Bouton", "box": {"x": -4, "y": 8, "width": 40, "height": 20}, "style": {"color": "rgb(255, 255, 255)"}},
                {"type": "svg", "markup": "<svg viewBox=\"0 0 1 1\"></svg>", "box": {"x": 60, "y": 8, "width": 16, "height": 16}}
            ]
        }
    }"#;

    let snapshot = parse_snapshot(json).unwrap();

    assert_eq!(snapshot.version, 1);
    let source = snapshot.source.unwrap();
    assert_eq!(source.package, "@mui/material");
    assert_eq!(source.state, "hover");
    assert_eq!(snapshot.viewport.width, 400.0);
    assert_eq!(snapshot.viewport.height, 200.0);
    assert_eq!(snapshot.viewport.device_scale_factor, 2.0);
    let DomNode::Element(root) = snapshot.root else { panic!("root should be an element") };
    assert_eq!(root.tag, "button");
    assert_eq!(root.label.as_deref(), Some("Bouton"));
    assert_eq!(root.r#box.x, 20.0);
    assert_eq!(root.r#box.width, 82.3);
    assert_eq!(root.r#box.height, 36.5);
    assert_eq!(root.style.get("backgroundColor").map(String::as_str), Some("rgb(25, 118, 210)"));
    assert_eq!(root.children.len(), 2);
    let DomNode::Text(text) = &root.children[0] else { panic!("first child should be text") };
    assert_eq!(text.text, "Bouton");
    assert_eq!(text.r#box.x, -4.0);
    assert_eq!(text.style.get("color").map(String::as_str), Some("rgb(255, 255, 255)"));
    let DomNode::Svg(svg) = &root.children[1] else { panic!("second child should be svg") };
    assert_eq!(svg.markup, "<svg viewBox=\"0 0 1 1\"></svg>");
    assert_eq!(svg.r#box.height, 16.0);
}

// covers: D1
#[test]
fn optional_fields_may_be_missing() {
    let json = snapshot_with_root(&format!(r#"{{"type":"element","tag":"div","box":{BOX}}}"#));

    let snapshot = parse_snapshot(&json).unwrap();

    assert!(snapshot.source.is_none());
    let DomNode::Element(root) = snapshot.root else { panic!("root should be an element") };
    assert!(root.label.is_none());
    assert!(root.style.is_empty());
    assert!(root.children.is_empty());
}

// covers: D2
#[test]
fn unsupported_version_is_refused_naming_the_version() {
    let json = snapshot_with_root(&format!(r#"{{"type":"element","tag":"div","box":{BOX}}}"#)).replace("\"version\":1", "\"version\":2");

    let message = error_message(&json);

    assert!(message.contains("version"), "{message}");
    assert!(message.contains('2'), "{message}");
}

// covers: D2
#[test]
fn malformed_json_is_refused_naming_json() {
    let message = error_message(r#"{"version":1,"#);

    assert!(message.contains("json"), "{message}");
}

// covers: D2
#[test]
fn unknown_node_type_is_refused_naming_the_type() {
    let json = snapshot_with_root(&format!(r#"{{"type":"video","box":{BOX}}}"#));

    let message = error_message(&json);

    assert!(message.contains("video"), "{message}");
}

// covers: D2
#[test]
fn oversized_json_is_refused_naming_bytes() {
    let padding = "a".repeat(MAX_JSON_BYTES);
    let json = snapshot_with_root(&format!(r#"{{"type":"element","tag":"div","label":"{padding}","box":{BOX}}}"#));
    assert!(json.len() > MAX_JSON_BYTES);

    let message = error_message(&json);

    assert!(message.contains("bytes"), "{message}");
}

// covers: D2
#[test]
fn too_many_nodes_are_refused_while_the_limit_itself_is_accepted() {
    assert!(parse_snapshot(&flat_nodes(MAX_NODES)).is_ok());

    let message = error_message(&flat_nodes(MAX_NODES + 1));

    assert!(message.contains("nodes"), "{message}");
}

// covers: D2
#[test]
fn too_deep_tree_is_refused_while_the_limit_itself_is_accepted() {
    assert!(parse_snapshot(&nested_elements(MAX_DEPTH)).is_ok());

    let message = error_message(&nested_elements(MAX_DEPTH + 1));

    assert!(message.contains("depth"), "{message}");
}

// covers: D2
#[test]
fn oversized_svg_markup_is_refused_naming_markup() {
    let markup = "a".repeat(MAX_MARKUP_BYTES + 1);
    let json =
        snapshot_with_root(&format!(r#"{{"type":"element","tag":"div","box":{BOX},"children":[{{"type":"svg","markup":"{markup}","box":{BOX}}}]}}"#));

    let message = error_message(&json);

    assert!(message.contains("markup"), "{message}");
}

// covers: D3
#[test]
fn negative_box_size_is_refused_naming_the_box() {
    let element = snapshot_with_root(r#"{"type":"element","tag":"div","box":{"x":0,"y":0,"width":-1,"height":10}}"#);
    let nested_text = snapshot_with_root(&format!(
        r#"{{"type":"element","tag":"div","box":{BOX},"children":[{{"type":"text","text":"a","box":{{"x":0,"y":0,"width":10,"height":-0.5}}}}]}}"#
    ));

    for json in [element, nested_text] {
        let message = error_message(&json);

        assert!(message.contains("box"), "{message}");
    }
}

// covers: D3
#[test]
fn non_finite_or_missing_box_values_are_refused() {
    let infinite = snapshot_with_root(r#"{"type":"element","tag":"div","box":{"x":0,"y":0,"width":1e400,"height":10}}"#);
    let missing_height = snapshot_with_root(r#"{"type":"element","tag":"div","box":{"x":0,"y":0,"width":10}}"#);
    let missing_box = snapshot_with_root(r#"{"type":"element","tag":"div"}"#);

    for json in [infinite, missing_height, missing_box] {
        assert!(parse_snapshot(&json).is_err(), "{json}");
    }
}
