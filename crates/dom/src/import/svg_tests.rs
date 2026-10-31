use figcraft_doc::NodeKind;
use serde_json::{Value, json};

use super::support::{Run, bounds, element_with, import};

const RED_SQUARE: &str =
    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><rect width="24" height="24" fill="#ff0000"/></svg>"##;

fn icon(markup: &str, width: f64, height: f64) -> Run {
    let svg = json!({ "type": "svg", "markup": markup, "box": bounds(30.0, 26.0, width, height) });
    import(element_with(json!({}), json!([svg])))
}

fn coefficients(run: &Run) -> [f64; 6] {
    run.child(0).transform.as_coeffs()
}

// covers: T8
#[test]
fn an_icon_sized_by_the_page_is_scaled_to_its_box() {
    let run = icon(RED_SQUARE, 12.0, 12.0);
    assert_eq!(coefficients(&run), [0.5, 0.0, 0.0, 0.5, 10.0, 6.0]);
    assert_eq!((run.child(0).width, run.child(0).height), (24.0, 24.0));
    assert!(run.warnings.is_empty(), "{:?}", run.warnings);
}

// covers: T8
#[test]
fn an_icon_stretches_on_each_axis_on_its_own() {
    assert_eq!(coefficients(&icon(RED_SQUARE, 48.0, 12.0)), [2.0, 0.0, 0.0, 0.5, 10.0, 6.0]);
}

// covers: T8
#[test]
fn an_icon_of_the_size_of_its_box_is_not_scaled() {
    assert_eq!(coefficients(&icon(RED_SQUARE, 24.0, 24.0)), [1.0, 0.0, 0.0, 1.0, 10.0, 6.0]);
    assert_eq!(coefficients(&icon(RED_SQUARE, 24.0004, 23.9996)), [1.0, 0.0, 0.0, 1.0, 10.0, 6.0]);
}

// covers: T8
#[test]
fn markup_straight_from_outer_html_has_no_namespace_and_is_read() {
    let markup = r#"<svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="6" fill="white"></circle></svg>"#;
    let run = icon(markup, 16.0, 16.0);
    assert!(!run.child(0).children.is_empty());
    assert!(matches!(run.child(0).kind, NodeKind::Frame));
    assert!(run.warnings.is_empty(), "{:?}", run.warnings);
}

// covers: T9
#[test]
fn markup_that_cannot_be_read_is_reported_and_the_import_goes_on() {
    let broken = json!({ "type": "svg", "markup": "<svg", "box": bounds(30.0, 26.0, 10.0, 10.0) });
    let sibling = json!({ "type": "element", "tag": "p", "box": bounds(20.0, 20.0, 5.0, 5.0) });
    let run = import(element_with(json!({}), json!([broken, sibling])));
    assert_eq!(run.root_node().children.len(), 1);
    assert_eq!(run.child(0).name, "p");
    assert!(run.warnings.iter().any(|warning| warning.starts_with("svg: markup ")), "{:?}", run.warnings);
}

// covers: T9
#[test]
fn what_the_vector_import_leaves_out_is_reported() {
    let markup = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><defs><filter id="f"><feColorMatrix type="saturate" values="0"/></filter></defs><rect width="10" height="10" fill="#ff0000" filter="url(#f)"/></svg>"##;
    let run = icon(markup, 10.0, 10.0);
    assert_eq!(run.root_node().children.len(), 1);
    assert!(run.warnings.iter().any(|warning| warning.starts_with("svg: ") && warning.contains("filter")), "{:?}", run.warnings);
}

// covers: T8
#[test]
fn an_icon_whose_box_has_no_area_is_not_drawn() {
    for (width, height) in [(0.0, 10.0), (10.0, 0.0), (0.0, 0.0)] {
        let run = icon(RED_SQUARE, width, height);
        assert!(run.root_node().children.is_empty(), "{width}x{height}");
        assert!(run.warnings.is_empty(), "{width}x{height}: {:?}", run.warnings);
    }
}

// covers: T8
#[test]
fn an_svg_can_be_the_root() {
    let svg: Value = json!({ "type": "svg", "markup": RED_SQUARE, "box": bounds(5.0, 5.0, 24.0, 24.0) });
    let run = import(svg);
    assert_eq!(run.root_node().transform.as_coeffs(), [1.0, 0.0, 0.0, 1.0, 100.0, 50.0]);
}

#[test]
fn a_rotated_icon_turns_about_its_centre() {
    let markup = "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><rect width='24' height='24' fill='red'/></svg>";
    let run = super::support::import(
        serde_json::json!({ "type": "svg", "markup": markup, "rotation": 180.0, "box": super::support::bounds(0.0, 0.0, 24.0, 24.0) }),
    );
    let [a, b, c, d, e, f] = run.root_node().transform.as_coeffs();
    assert!((a + 1.0).abs() < 1e-9 && b.abs() < 1e-9 && c.abs() < 1e-9 && (d + 1.0).abs() < 1e-9);
    assert!((e - 124.0).abs() < 1e-9 && (f - 74.0).abs() < 1e-9, "{e} {f}");
}
