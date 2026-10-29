use super::*;
use figcraft_color::Color;
use figcraft_doc::{Document, EffectKind, Node, NodeId, NodeKind, StrokeAlign};
use serde_json::{Value, json};

fn element(style: Value, children: Value) -> Value {
    json!({
        "type": "element",
        "tag": "button",
        "label": "Bouton",
        "box": { "x": 20, "y": 20, "width": 92, "height": 36 },
        "style": style,
        "children": children
    })
}

fn import(root: Value) -> (Document, NodeId, Vec<String>) {
    let snapshot =
        parse_snapshot(&json!({ "version": 1, "viewport": { "width": 400, "height": 200, "deviceScaleFactor": 1 }, "root": root }).to_string())
            .unwrap();
    let mut doc = Document::new("t");
    let page = doc.first_page().unwrap();
    let imported = import_snapshot(&mut doc, &snapshot, page, 100.0, 50.0).unwrap();
    (doc, imported.root, imported.warnings)
}

fn import_style(style: Value) -> (Document, NodeId, Vec<String>) {
    import(element(style, json!([])))
}

fn offset(node: &Node) -> (f64, f64) {
    let c = node.transform.as_coeffs();
    (c[4], c[5])
}

fn near(actual: [u8; 4], expected: [u8; 4]) -> bool {
    actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 1)
}

fn fill_rgba(node: &Node) -> Vec<[u8; 4]> {
    node.fills.iter().filter_map(|p| p.solid_color()).map(Color::to_rgba8).collect()
}

fn border(widths: [&str; 4], style: &str, color: &str) -> Value {
    let mut map = serde_json::Map::new();
    for (side, width) in ["Top", "Right", "Bottom", "Left"].into_iter().zip(widths) {
        map.insert(format!("border{side}Width"), json!(width));
        map.insert(format!("border{side}Style"), json!(style));
        map.insert(format!("border{side}Color"), json!(color));
    }
    Value::Object(map)
}

fn subtree(doc: &Document, id: NodeId) -> Vec<NodeId> {
    let mut all = vec![id];
    for child in &doc.get(id).unwrap().children {
        all.extend(subtree(doc, *child));
    }
    all
}

// covers: T1
#[test]
fn element_becomes_a_frame_at_the_requested_position() {
    let (doc, root, warnings) = import_style(json!({}));
    let node = doc.get(root).unwrap();
    assert_eq!(node.kind, NodeKind::Frame);
    assert_eq!(node.name, "Bouton");
    assert_eq!((node.width, node.height), (92.0, 36.0));
    assert_eq!(offset(node), (100.0, 50.0));
    assert!(warnings.is_empty(), "{warnings:?}");
}

// covers: T1
#[test]
fn element_without_label_is_named_after_its_tag() {
    let root = json!({ "type": "element", "tag": "section", "box": { "x": 0, "y": 0, "width": 10, "height": 10 } });
    let (doc, root, _) = import(root);
    assert_eq!(doc.get(root).unwrap().name, "section");
}

// covers: T1
#[test]
fn child_is_positioned_relative_to_its_parent() {
    let child = json!({ "type": "element", "tag": "span", "box": { "x": 30, "y": 26, "width": 40, "height": 10 } });
    let (doc, root, _) = import(element(json!({}), json!([child])));
    let parent = doc.get(root).unwrap();
    assert_eq!(parent.children.len(), 1);
    let node = doc.get(parent.children[0]).unwrap();
    assert_eq!(node.name, "span");
    assert_eq!(offset(node), (10.0, 6.0));
    assert_eq!((node.width, node.height), (40.0, 10.0));
}

// covers: T2
#[test]
fn opaque_and_translucent_backgrounds_become_solid_fills() {
    let (doc, root, _) = import_style(json!({ "backgroundColor": "rgb(25, 118, 210)" }));
    assert_eq!(fill_rgba(doc.get(root).unwrap()), vec![[25, 118, 210, 255]]);

    let (doc, root, _) = import_style(json!({ "backgroundColor": "rgba(0, 0, 0, 0.5)" }));
    let fills = fill_rgba(doc.get(root).unwrap());
    assert_eq!(fills.len(), 1);
    assert!(near(fills[0], [0, 0, 0, 128]), "{fills:?}");
}

// covers: T2
#[test]
fn transparent_background_gives_no_fill() {
    for value in ["transparent", "rgba(0, 0, 0, 0)"] {
        let (doc, root, warnings) = import_style(json!({ "backgroundColor": value }));
        assert!(doc.get(root).unwrap().fills.is_empty(), "{value}");
        assert!(warnings.is_empty(), "{value}: {warnings:?}");
    }
}

// covers: T3
#[test]
fn four_equal_solid_borders_become_an_inside_stroke() {
    let (doc, root, _) = import_style(border(["2px"; 4], "solid", "rgb(255, 0, 0)"));
    let node = doc.get(root).unwrap();
    let strokes: Vec<[u8; 4]> = node.strokes.iter().filter_map(|p| p.solid_color()).map(Color::to_rgba8).collect();
    assert_eq!(strokes, vec![[255, 0, 0, 255]]);
    assert_eq!(node.stroke.align, StrokeAlign::Inside);
    assert_eq!(node.stroke.sides.unwrap_or([node.stroke.weight; 4]), [2.0; 4]);
}

// covers: T3
#[test]
fn different_border_widths_become_per_side_weights() {
    let (doc, root, _) = import_style(border(["1px", "2px", "3px", "4px"], "solid", "rgb(0, 0, 255)"));
    let node = doc.get(root).unwrap();
    assert_eq!(node.strokes.len(), 1);
    assert_eq!(node.stroke.sides, Some([1.0, 2.0, 3.0, 4.0]));
}

// covers: T3
#[test]
fn none_style_or_zero_width_gives_no_stroke() {
    let (doc, root, _) = import_style(border(["3px"; 4], "none", "rgb(255, 0, 0)"));
    assert!(doc.get(root).unwrap().strokes.is_empty());
    let (doc, root, _) = import_style(border(["0px"; 4], "solid", "rgb(255, 0, 0)"));
    assert!(doc.get(root).unwrap().strokes.is_empty());
}

// covers: T4
#[test]
fn four_corner_radii_are_taken_in_pixels() {
    let (doc, root, _) = import_style(json!({
        "borderTopLeftRadius": "4px",
        "borderTopRightRadius": "8px",
        "borderBottomRightRadius": "12px",
        "borderBottomLeftRadius": "16px"
    }));
    assert_eq!(doc.get(root).unwrap().corners.radii, [4.0, 8.0, 12.0, 16.0]);
}

// covers: T5
#[test]
fn box_shadow_list_becomes_drop_and_inner_shadows() {
    let (doc, root, _) = import_style(json!({
        "boxShadow": "0px 2px 4px -1px rgba(0, 0, 0, 0.2), inset 0px 0px 0px 1px rgb(255, 0, 0)"
    }));
    let effects = &doc.get(root).unwrap().effects;
    assert_eq!(effects.len(), 2);

    let drop = effects.iter().find_map(|e| if let EffectKind::DropShadow(s) = &e.kind { Some(s) } else { None }).unwrap();
    assert_eq!(drop.offset, (0.0, 2.0));
    assert_eq!((drop.radius, drop.spread), (4.0, -1.0));
    assert!(near(drop.color.to_rgba8(), [0, 0, 0, 51]), "{:?}", drop.color);

    let inner = effects.iter().find_map(|e| if let EffectKind::InnerShadow(s) = &e.kind { Some(s) } else { None }).unwrap();
    assert_eq!(inner.offset, (0.0, 0.0));
    assert_eq!((inner.radius, inner.spread), (0.0, 1.0));
    assert!(near(inner.color.to_rgba8(), [255, 0, 0, 255]), "{:?}", inner.color);
}

// covers: T6
#[test]
fn opacity_is_kept_and_overflow_hidden_clips() {
    let (doc, root, _) = import_style(json!({ "opacity": "0.5", "overflow": "hidden" }));
    let node = doc.get(root).unwrap();
    assert!((node.opacity - 0.5).abs() < 1e-6, "{}", node.opacity);
    assert!(node.clip);

    let (doc, root, _) = import_style(json!({ "overflow": "visible" }));
    let node = doc.get(root).unwrap();
    assert_eq!(node.opacity, 1.0);
    assert!(!node.clip);
}

// covers: T6
#[test]
fn display_none_and_visibility_hidden_nodes_are_skipped() {
    let hidden = |style: Value| json!({ "type": "element", "tag": "i", "box": { "x": 20, "y": 20, "width": 5, "height": 5 }, "style": style });
    let shown = json!({ "type": "element", "tag": "b", "box": { "x": 20, "y": 20, "width": 5, "height": 5 } });
    let children = json!([hidden(json!({ "display": "none" })), hidden(json!({ "visibility": "hidden" })), shown]);
    let (doc, root, _) = import(element(json!({}), children));
    let parent = doc.get(root).unwrap();
    assert_eq!(parent.children.len(), 1);
    assert_eq!(doc.get(parent.children[0]).unwrap().name, "b");
}

// covers: T7
#[test]
fn text_becomes_a_text_layer_at_its_box() {
    let text = json!({
        "type": "text",
        "text": "Bouton",
        "box": { "x": 30, "y": 26, "width": 50, "height": 20 },
        "style": {
            "fontFamily": "\"Roboto\", \"Helvetica\", \"Arial\", sans-serif",
            "fontSize": "14px",
            "fontWeight": "700",
            "fontStyle": "italic",
            "lineHeight": "20px",
            "letterSpacing": "0.5px",
            "color": "rgb(255, 255, 255)",
            "textTransform": "uppercase"
        }
    });
    let (doc, root, warnings) = import(element(json!({}), json!([text])));
    let parent = doc.get(root).unwrap();
    assert_eq!(parent.children.len(), 1);
    let node = doc.get(parent.children[0]).unwrap();
    assert_eq!(offset(node), (10.0, 6.0));
    assert_eq!((node.width, node.height), (50.0, 20.0));
    assert_eq!(fill_rgba(node), vec![[255, 255, 255, 255]]);
    let NodeKind::Text(data) = &node.kind else { panic!("expected a text layer, got {:?}", node.kind) };
    assert_eq!(data.characters, "BOUTON");
    assert_eq!(data.style.font_family, "Roboto");
    assert_eq!(data.style.font_size, 14.0);
    assert_eq!(data.style.weight(), 700);
    assert!(data.style.italic());
    assert!((data.style.line_height.resolve(14.0, 1.2) - 20.0).abs() < 1e-6);
    assert!((data.style.letter_spacing.resolve(14.0) - 0.5).abs() < 1e-6);
    assert!(warnings.is_empty(), "{warnings:?}");
}

// covers: T8
#[test]
fn svg_becomes_an_imported_vector_layer_at_its_box() {
    let svg = json!({
        "type": "svg",
        "markup": "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><rect width=\"10\" height=\"10\" fill=\"#ff0000\"/></svg>",
        "box": { "x": 30, "y": 26, "width": 10, "height": 10 }
    });
    let (doc, root, warnings) = import(element(json!({}), json!([svg])));
    let parent = doc.get(root).unwrap();
    assert_eq!(parent.children.len(), 1);
    let node = doc.get(parent.children[0]).unwrap();
    assert_eq!(offset(node), (10.0, 6.0));
    assert_eq!((node.width, node.height), (10.0, 10.0));
    assert!(!matches!(node.kind, NodeKind::Text(_)));
    let red = subtree(&doc, parent.children[0]).into_iter().any(|id| fill_rgba(doc.get(id).unwrap()).contains(&[255, 0, 0, 255]));
    assert!(red, "the svg's red rectangle is imported");
    assert!(warnings.is_empty(), "{warnings:?}");
}

// covers: T9
#[test]
fn unreadable_value_is_skipped_and_named_in_warnings() {
    let sibling = json!({
        "type": "element", "tag": "p", "box": { "x": 20, "y": 20, "width": 5, "height": 5 },
        "style": { "backgroundColor": "rgb(0, 128, 0)" }
    });
    let style = json!({ "backgroundColor": "nonsense(1, 2)", "borderTopLeftRadius": "wide" });
    let (doc, root, warnings) = import(element(style, json!([sibling])));
    let node = doc.get(root).unwrap();
    assert!(node.fills.is_empty(), "nothing is invented");
    assert_eq!(node.corners.radii, [0.0; 4]);
    for property in ["backgroundColor", "borderTopLeftRadius"] {
        assert!(warnings.iter().any(|w| w.contains("Bouton") && w.contains(property)), "{property}: {warnings:?}");
    }
    assert_eq!(node.children.len(), 1, "the import went on");
    assert_eq!(fill_rgba(doc.get(node.children[0]).unwrap()), vec![[0, 128, 0, 255]]);
}
