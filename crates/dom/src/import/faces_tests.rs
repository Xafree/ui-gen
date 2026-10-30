use serde_json::{Value, json};

use super::support::{bounds, element_with, snapshot};
use crate::text_faces;

fn text(label: &str, style: Value) -> Value {
    json!({ "type": "text", "text": label, "box": bounds(30.0, 26.0, 10.0, 10.0), "lineBox": bounds(29.0, 25.0, 12.0, 12.0), "style": style })
}

// covers: E5-1.1
#[test]
fn a_text_face_is_the_platform_font_else_the_first_family_of_the_stack_in_tree_order() {
    let nested = json!({ "type": "element", "tag": "span", "box": bounds(20.0, 20.0, 5.0, 5.0), "children": [text("b", json!({ "fontFamily": "\"Inter\", Arial", "fontSize": "20px", "fontWeight": "bold" }))] });
    let root = element_with(
        json!({}),
        json!([
            text("a", json!({ "fontFamily": "Georgia", "platformFont": "Segoe UI", "fontSize": "14px", "fontWeight": "500" })),
            nested,
            text("c", json!({}))
        ]),
    );

    let faces = text_faces(&snapshot(root));

    let found: Vec<(&str, f64, u16)> = faces.iter().map(|face| (face.family.as_str(), face.size, face.weight)).collect();
    assert_eq!(found, [("Segoe UI", 14.0, 500), ("Inter", 20.0, 700), ("", 16.0, 400)]);
    assert!(faces.iter().all(|face| face.line_box.is_some()));
}

// covers: E5-1.1
#[test]
fn hidden_elements_and_unreadable_sizes_are_not_faces_of_their_own() {
    let hidden = json!({ "type": "element", "tag": "div", "box": bounds(20.0, 20.0, 5.0, 5.0), "style": { "display": "none" }, "children": [text("gone", json!({}))] });
    let root = element_with(json!({}), json!([hidden, text("kept", json!({ "fontSize": "NaNpx", "fontWeight": "NaN" }))]));

    let faces = text_faces(&snapshot(root));

    assert_eq!(faces.len(), 1);
    assert_eq!((faces[0].size, faces[0].weight), (16.0, 400));
}
