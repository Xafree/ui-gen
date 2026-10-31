use figcraft_doc::{Document, LineHeight, Node, NodeId, NodeKind, TextData};
use serde_json::{Value, json};

use super::fonts::nearest_style;
use super::support::{bounds, snapshot};
use crate::{import_snapshot, nearest_weight};

const NO_SUCH_FONT: &str = "NoSuchFontForUigenTests";

struct Placed {
    doc: Document,
    root: NodeId,
    warnings: Vec<String>,
}

impl Placed {
    fn root_node(&self) -> &Node {
        self.node(self.root)
    }

    fn node(&self, id: NodeId) -> &Node {
        self.doc.get(id).unwrap()
    }

    fn child(&self, parent: &Node, index: usize) -> &Node {
        self.node(*parent.children.get(index).unwrap())
    }
}

fn place(root: Value) -> Placed {
    let snapshot = snapshot(root);
    let mut doc = Document::new("t");
    let page = doc.first_page().unwrap();
    let imported = import_snapshot(&mut doc, &snapshot, page, 0.0, 0.0).unwrap();
    Placed { doc, root: imported.root, warnings: imported.warnings }
}

fn parent_of(children: Value) -> Value {
    json!({ "type": "element", "tag": "p", "label": "Parent", "box": bounds(0.0, 0.0, 100.0, 50.0), "children": children })
}

fn text_node(text_box: Value, line_box: Option<Value>, style: Value) -> Value {
    let mut node = json!({ "type": "text", "text": "Bouton", "box": text_box, "style": style });
    if let Some(line_box) = line_box {
        node["lineBox"] = line_box;
    }
    node
}

fn text_data(node: &Node) -> &TextData {
    match &node.kind {
        NodeKind::Text(data) => data,
        other => panic!("a text layer was expected, got {other:?}"),
    }
}

fn offset(node: &Node) -> (f64, f64) {
    let coefficients = node.transform.as_coeffs();
    (coefficients[4], coefficients[5])
}

fn text_style_of(style: Value) -> (String, String) {
    let placed = place(parent_of(json!([text_node(bounds(0.0, 0.0, 40.0, 17.0), None, style)])));
    let data = text_data(placed.child(placed.root_node(), 0));
    (data.style.font_family.clone(), data.style.font_style.clone())
}

// covers: P2
#[test]
fn the_platform_font_is_used_as_it_is_whatever_the_stack_says() {
    let family = "Zzz Platform Face";
    for stack in [format!("\"{NO_SUCH_FONT}\", system-ui"), "Arial, sans-serif".to_owned(), String::new()] {
        let (used, _) = text_style_of(json!({ "fontFamily": stack, "platformFont": family }));
        assert_eq!(used, family, "stack: {stack:?}");
    }
}

// covers: P2
#[test]
fn a_stack_with_no_installed_family_keeps_its_first_family() {
    let stack = format!("\"{NO_SUCH_FONT}A\", '{NO_SUCH_FONT}B', monospace");
    let (used, _) = text_style_of(json!({ "fontFamily": stack }));
    assert_eq!(used, format!("{NO_SUCH_FONT}A"));
}

// covers: P2
#[cfg(windows)]
#[test]
fn the_first_installed_family_of_the_stack_is_used() {
    figcraft_text::scan_system_fonts();
    let stack = format!("\"{NO_SUCH_FONT}\", 'Segoe UI', serif");
    let (used, _) = text_style_of(json!({ "fontFamily": stack }));
    assert_eq!(used, "Segoe UI");
}

// covers: P2
#[cfg(windows)]
#[test]
fn the_system_ui_keywords_resolve_to_the_interface_font_of_the_platform() {
    figcraft_text::scan_system_fonts();
    for keyword in ["system-ui", "-apple-system", "BlinkMacSystemFont"] {
        let stack = format!("\"{NO_SUCH_FONT}\", {keyword}, sans-serif");
        let (used, _) = text_style_of(json!({ "fontFamily": stack }));
        assert_eq!(used, "Segoe UI", "{keyword}");
    }
    let (used, _) = text_style_of(json!({ "fontFamily": "system-ui" }));
    assert_eq!(used, "Segoe UI");
}

// covers: P2
#[test]
fn the_nearest_available_weight_is_chosen_never_a_synthesised_one() {
    let faces = [400, 700];
    for (wanted, expected) in [(400, 400), (700, 700), (500, 400), (600, 700), (300, 400), (100, 400), (900, 700), (1000, 700)] {
        assert_eq!(nearest_weight(&faces, wanted), expected, "wanted {wanted}");
    }
    assert_eq!(nearest_weight(&[700, 300, 400], 320), 300, "unsorted faces");
    assert_eq!(nearest_weight(&[300, 600, 900], 800), 900);
    assert_eq!(nearest_weight(&[500], 100), 500);
    assert_eq!(nearest_weight(&[500], 900), 500);
}

// covers: P2
#[test]
fn without_any_face_to_choose_from_the_wanted_weight_is_kept() {
    assert_eq!(nearest_weight(&[], 500), 500);
    let (_, style) = text_style_of(json!({ "platformFont": "Zzz Platform Face", "fontWeight": "500" }));
    assert_eq!(style, "Medium");
}

// covers: P2
#[cfg(windows)]
#[test]
fn the_style_of_an_installed_family_is_one_of_its_styles() {
    figcraft_text::scan_system_fonts();
    let styles = figcraft_text::family_styles("Segoe UI");
    assert!(!styles.is_empty(), "Segoe UI is expected on Windows");
    for weight in ["100", "300", "400", "500", "600", "700", "800", "900"] {
        for italic in ["normal", "italic"] {
            let (family, style) = text_style_of(json!({ "platformFont": "Segoe UI", "fontWeight": weight, "fontStyle": italic }));
            assert_eq!(family, "Segoe UI");
            assert!(styles.contains(&style), "weight {weight} {italic}: {style:?} is not in {styles:?}");
        }
    }
    let (_, regular) = text_style_of(json!({ "platformFont": "Segoe UI", "fontWeight": "400" }));
    let (_, bold) = text_style_of(json!({ "platformFont": "Segoe UI", "fontWeight": "700" }));
    assert_eq!((regular.as_str(), bold.as_str()), ("Regular", "Bold"));
}

fn names(styles: &[&str]) -> Vec<String> {
    styles.iter().map(|style| (*style).to_owned()).collect()
}

// covers: P2
#[test]
fn a_narrower_or_wider_cut_is_never_chosen_for_a_family_that_has_the_normal_one() {
    let arial = names(&["Narrow", "Regular", "Italic", "Narrow Italic", "Bold", "Narrow Bold", "Bold Italic", "Black"]);
    for (weight, italic, expected) in [
        (400, false, "Regular"),
        (500, false, "Regular"),
        (700, false, "Bold"),
        (400, true, "Italic"),
        (700, true, "Bold Italic"),
        (900, false, "Black"),
    ] {
        assert_eq!(nearest_style(&arial, weight, italic).as_deref(), Some(expected), "{weight} {italic}");
    }
    let wide = names(&["Condensed", "Expanded", "Semi Condensed", "Extra Expanded", "Regular"]);
    assert_eq!(nearest_style(&wide, 400, false).as_deref(), Some("Regular"));
}

// covers: P2
#[test]
fn a_family_that_only_has_a_narrow_cut_keeps_it() {
    let only = names(&["Condensed", "Condensed Bold"]);
    assert_eq!(nearest_style(&only, 700, false).as_deref(), Some("Condensed Bold"));
    assert_eq!(nearest_style(&[], 700, false), None);
}

// covers: P2
#[cfg(windows)]
#[test]
fn arial_of_the_platform_is_drawn_in_its_regular_cut() {
    figcraft_text::scan_system_fonts();
    let (family, style) =
        text_style_of(json!({ "fontFamily": "Arial", "platformFont": "Arial", "platformFontPostScript": "ArialMT", "fontWeight": "400" }));
    assert_eq!((family.as_str(), style.as_str()), ("Arial", "Regular"), "{:?}", figcraft_text::family_styles("Arial"));
}

// covers: P2
#[cfg(windows)]
#[test]
fn a_bare_button_in_arial_is_as_wide_as_in_the_browser() {
    figcraft_text::scan_system_fonts();
    let style = json!({ "fontFamily": "Arial", "platformFont": "Arial", "fontSize": "13.3333px", "fontWeight": "400" });
    let placed = place(parent_of(json!([text_node(bounds(0.0, 0.0, 38.5, 15.0), None, style)])));
    let text = text_data(placed.child(placed.root_node(), 0));
    let width = figcraft_text::layout_cached(&TextData { characters: "Button".to_owned(), ..text.clone() }, None, None).width;
    assert!((width - 38.5).abs() < 0.5, "Chromium measures 38.5 px, got {width}");
}

// covers: P3
#[test]
fn a_text_layer_sits_on_its_line_box_and_takes_its_height_as_line_height() {
    let text = text_node(bounds(5.0, 13.5, 60.0, 17.0), Some(bounds(5.0, 10.0, 60.0, 24.0)), json!({ "fontSize": "14px", "lineHeight": "24px" }));
    let placed = place(parent_of(json!([text])));
    let node = placed.child(placed.root_node(), 0);
    assert_eq!(offset(node), (5.0, 10.0));
    assert_eq!((node.width, node.height), (60.0, 24.0));
    assert_eq!(text_data(node).style.line_height, LineHeight::Pixels(24.0));
    assert!(placed.warnings.is_empty(), "{:?}", placed.warnings);
}

// covers: P3
#[test]
fn the_line_box_height_is_the_line_height_even_when_the_style_says_normal() {
    let text = text_node(bounds(5.0, 13.5, 60.0, 17.0), Some(bounds(5.0, 10.0, 60.0, 24.0)), json!({ "fontSize": "14px", "lineHeight": "normal" }));
    let placed = place(parent_of(json!([text])));
    let node = placed.child(placed.root_node(), 0);
    assert_eq!(offset(node).1, 10.0);
    assert_eq!(text_data(node).style.line_height, LineHeight::Pixels(24.0));
}

// covers: P3
#[test]
fn without_a_line_box_the_text_layer_keeps_its_own_box_and_line_height() {
    let text = text_node(bounds(5.0, 13.5, 60.0, 17.0), None, json!({ "fontSize": "14px", "lineHeight": "20px" }));
    let placed = place(parent_of(json!([text])));
    let node = placed.child(placed.root_node(), 0);
    assert_eq!(offset(node), (5.0, 14.0));
    assert_eq!((node.width, node.height), (60.0, 17.0));
    assert_eq!(text_data(node).style.line_height, LineHeight::Pixels(20.0));
}

// covers: P3
#[test]
fn a_line_box_is_snapped_to_pixels_like_any_other_box() {
    let text = text_node(bounds(5.4, 13.5, 59.6, 17.0), Some(bounds(5.4, 9.6, 59.6, 24.0)), json!({ "lineHeight": "24px" }));
    let placed = place(parent_of(json!([text])));
    let node = placed.child(placed.root_node(), 0);
    assert_eq!(offset(node), (5.0, 10.0));
    assert_eq!((node.width, node.height), (60.0, 24.0));
}

// covers: P4
#[test]
fn boxes_are_snapped_to_pixels_from_their_rounded_edges() {
    let root = json!({ "type": "element", "tag": "button", "label": "Bouton", "box": bounds(20.0, 20.0, 82.3, 36.5), "children": [] });
    let placed = place(root);
    let node = placed.root_node();
    assert_eq!((node.width, node.height), (82.0, 37.0));
    assert_eq!(offset(node), (0.0, 0.0));
}

// covers: P4
#[test]
fn relative_positions_are_recomputed_from_the_rounded_edges_of_each_level() {
    let grandchild = json!({ "type": "element", "tag": "i", "box": bounds(36.4, 20.0, 5.0, 5.0) });
    let child = json!({ "type": "element", "tag": "b", "box": bounds(33.6, 20.0, 10.4, 10.0), "children": [grandchild] });
    let root = json!({ "type": "element", "tag": "button", "box": bounds(20.0, 20.0, 82.3, 36.5), "children": [child] });
    let placed = place(root);
    let child = placed.child(placed.root_node(), 0);
    assert_eq!(offset(child), (14.0, 0.0), "round(33.6) - round(20)");
    assert_eq!((child.width, child.height), (10.0, 10.0), "round(44.0) - round(33.6)");
    let grandchild = placed.child(child, 0);
    assert_eq!(offset(grandchild), (2.0, 0.0), "round(36.4) - round(33.6), not 36.4 - 33.6");
    assert_eq!((grandchild.width, grandchild.height), (5.0, 5.0), "round(41.4) - round(36.4)");
}

// covers: P4
#[test]
fn text_and_svg_boxes_are_snapped_too() {
    let text = text_node(bounds(5.4, 13.5, 40.4, 17.3), None, json!({}));
    let markup =
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><rect width="24" height="24" fill="#ff0000"/></svg>"##;
    let svg = json!({ "type": "svg", "markup": markup, "box": bounds(60.4, 26.4, 12.2, 12.2) });
    let placed = place(parent_of(json!([text, svg])));
    let text = placed.child(placed.root_node(), 0);
    assert_eq!(offset(text), (5.0, 14.0));
    assert_eq!((text.width, text.height), (41.0, 17.0), "round(45.8) - round(5.4), round(30.8) - round(13.5)");
    let [a, b, c, d, e, f] = placed.child(placed.root_node(), 1).transform.as_coeffs();
    let scale = 13.0 / 24.0;
    assert!((a - scale).abs() < 1e-9 && (d - scale).abs() < 1e-9 && b == 0.0 && c == 0.0, "scaled to 13 x 13: {a} {d}");
    assert_eq!((e, f), (60.0, 26.0));
}

// covers: P4
#[test]
fn a_box_thinner_than_a_pixel_collapses_to_zero_never_below() {
    let hairline = json!({ "type": "element", "tag": "hr", "box": bounds(10.1, 30.0, 0.2, 10.0) });
    let placed = place(parent_of(json!([hairline])));
    let node = placed.child(placed.root_node(), 0);
    assert_eq!(offset(node), (10.0, 30.0));
    assert_eq!((node.width, node.height), (0.0, 10.0));
}

#[test]
fn a_text_remembers_the_font_stack_the_library_asked_for() {
    let stack = "system-ui, \"Segoe UI\", sans-serif";
    let placed = place(parent_of(json!([text_node(bounds(0.0, 0.0, 40.0, 17.0), None, json!({"fontFamily": stack}))])));
    let label = placed.child(placed.root_node(), 0);
    assert_eq!(label.plugin_data.get(super::text::FONT_STACK_KEY).map(String::as_str), Some(stack));
}
