//! Lit review fixes: state rules that hold whatever the order of the variants, ids of inline SVGs,
//! notes for frozen vector colours, class name collisions and the cap on variants.

use std::collections::BTreeMap;

use crate::Session;
use crate::components::tests::{ex, idv};
use figcraft_doc::*;
use serde_json::{Value, json};

fn component_code(s: &mut Session, id: NodeId) -> Value {
    ex(s, "inspect.componentCode", json!({"id": id, "language": "lit"}))
}

fn definition(s: &mut Session, id: NodeId) -> String {
    component_code(s, id)["definition"].as_str().unwrap_or_default().to_string()
}

/// A variant row: name, background, border width (0: none).
type Row = (&'static str, &'static str, f64);

fn bordered_set(s: &mut Session, rows: &[Row]) -> NodeId {
    let mut variants = Vec::new();
    for (k, (variant, fill, border)) in rows.iter().enumerate() {
        let f = idv(&ex(s, "frame.create", json!({"x": k as f64 * 150.0, "y": 0, "width": 100, "height": 40, "name": variant})));
        let mut props = json!({"name": "Bg", "fill": fill});
        if *border > 0.0 {
            props["stroke"] = json!("#000000");
            props["strokeWeight"] = json!(border);
        }
        ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": props}));
        variants.push(idv(&ex(s, "component.create", json!({"ids": [f]}))));
    }
    let set = idv(&ex(s, "component.combineVariants", json!({"ids": variants})));
    ex(s, "node.set", json!({"id": set, "props": {"name": "Control"}}));
    set
}

/// What a browser would show: the CSS rules of a generated definition, applied by specificity
/// then source order to an element in a given state.
type Rule = (Option<String>, String, Vec<(String, String)>);

struct Page {
    rules: Vec<Rule>,
}

#[derive(Default)]
struct Element {
    attributes: BTreeMap<&'static str, &'static str>,
    hover: bool,
    active: bool,
}

impl Page {
    fn parse(def: &str) -> Page {
        let mut rules = Vec::new();
        let mut open: Option<Rule> = None;
        for line in def.lines() {
            let line = line.trim();
            if let Some(rule) = open.as_mut() {
                if line == "}" {
                    rules.extend(open.take());
                } else if let Some((p, v)) = line.split_once(':') {
                    rule.2.push((p.trim().to_string(), v.trim().trim_end_matches(';').to_string()));
                }
            } else if let Some(head) = line.strip_suffix(" {")
                && let Some(inner) = head.strip_prefix(":host(")
                && let Some((host, class)) = inner.rsplit_once(") .")
            {
                open = Some((Some(host.to_string()), class.to_string(), Vec::new()));
            } else if let Some(class) = line.strip_suffix(" {").and_then(|h| h.strip_prefix(".")) {
                open = Some((None, class.to_string(), Vec::new()));
            }
        }
        Page { rules }
    }

    /// How many simple selectors of `host` match `el`, or `None` when one does not.
    fn matches(host: &str, el: &Element) -> Option<usize> {
        let mut rest = host;
        let mut count = 1;
        while !rest.is_empty() {
            if let Some(after) = rest.strip_prefix('[') {
                let (inside, tail) = after.split_once(']')?;
                let ok = match inside.split_once('=') {
                    Some((name, value)) => el.attributes.get(name) == Some(&value.trim_matches('"')),
                    None => el.attributes.contains_key(inside),
                };
                if !ok {
                    return None;
                }
                rest = tail;
            } else if let Some(tail) = rest.strip_prefix(":hover") {
                el.hover.then_some(())?;
                rest = tail;
            } else {
                let tail = rest.strip_prefix(":active")?;
                el.active.then_some(())?;
                rest = tail;
            }
            count += 1;
        }
        Some(count)
    }

    /// The final value of `property` on layer `class`.
    fn value(&self, class: &str, property: &str, el: &Element) -> Option<String> {
        let mut best: Option<(usize, usize, String)> = None;
        for (order, (host, c, decls)) in self.rules.iter().enumerate() {
            let Some(specificity) = host.as_deref().map_or(Some(0), |h| Page::matches(h, el)) else { continue };
            let Some((_, v)) = decls.iter().rev().find(|(p, _)| p == property) else { continue };
            if c == class && best.as_ref().is_none_or(|(s, o, _)| (specificity, order) > (*s, *o)) {
                best = Some((specificity, order, v.clone()));
            }
        }
        best.map(|(_, _, v)| v)
    }
}

fn element(style: &'static str, hover: bool, disabled: bool) -> Element {
    let mut attributes = BTreeMap::from([("style-variant", style)]);
    if disabled {
        attributes.insert("disabled", "");
    }
    Element { attributes, hover, active: false }
}

const STYLE_BY_STATE: [Row; 6] = [
    ("Style=Primary, State=Default", "#ff0000", 0.0),
    ("Style=Secondary, State=Default", "#00ff00", 1.0),
    ("Style=Primary, State=Hover", "#0000ff", 0.0),
    ("Style=Secondary, State=Hover", "#00ff00", 3.0),
    ("Style=Primary, State=Disabled", "#808080", 0.0),
    ("Style=Secondary, State=Disabled", "#cccccc", 1.0),
];

fn check_states(rows: &[Row]) {
    let mut s = Session::new();
    let set = bordered_set(&mut s, rows);
    let def = definition(&mut s, set);
    let page = Page::parse(&def);
    let bg = |el: &Element, property: &str| page.value("bg", property, el);

    assert_eq!(bg(&element("Primary", false, false), "background").as_deref(), Some("#FF0000"), "{def}");
    assert_eq!(bg(&element("Primary", true, false), "background").as_deref(), Some("#0000FF"), "{def}");
    assert_eq!(bg(&element("Secondary", true, false), "background").as_deref(), Some("#00FF00"), "Primary hover leaks:\n{def}");
    assert_eq!(bg(&element("Secondary", false, false), "background").as_deref(), Some("#00FF00"), "{def}");
    assert_eq!(bg(&element("Primary", true, true), "background").as_deref(), Some("#808080"), "a hovered disabled keeps its look:\n{def}");
    assert_eq!(bg(&element("Secondary", true, true), "background").as_deref(), Some("#CCCCCC"), "{def}");
    assert_eq!(bg(&element("Primary", false, true), "background").as_deref(), Some("#808080"), "{def}");

    let rest = bg(&element("Secondary", false, false), "border");
    let hovered = bg(&element("Secondary", true, false), "border");
    assert_ne!(rest, hovered, "Secondary hover changes its border:\n{def}");
    assert_eq!(bg(&element("Secondary", true, true), "border"), rest, "a hovered disabled has the border at rest:\n{def}");
}

#[test]
fn state_rules_hold_for_variants_grouped_by_state() {
    // covers: E6-2.4
    check_states(&STYLE_BY_STATE);
}

#[test]
fn state_rules_hold_for_variants_in_any_order() {
    // covers: E6-2.4
    let mut shuffled = STYLE_BY_STATE;
    shuffled.reverse();
    let default = shuffled.iter().position(|r| r.0 == "Style=Primary, State=Default").unwrap_or_default();
    shuffled.swap(0, default);
    check_states(&shuffled);
    let mut by_style = STYLE_BY_STATE;
    by_style.sort_by_key(|r| (r.0.contains("Secondary"), r.0.contains("Disabled"), r.0.contains("Hover")));
    check_states(&by_style);
}

fn notes_of(v: &Value) -> Vec<Value> {
    v["notes"].as_array().cloned().unwrap_or_default()
}

fn note_with<'a>(notes: &'a [Value], code: &str) -> Option<&'a Value> {
    notes.iter().find(|n| n["code"] == code)
}

fn plain_component(s: &mut Session, name: &str, x: f64) -> NodeId {
    let f = idv(&ex(s, "frame.create", json!({"x": x, "y": 0, "width": 40, "height": 40, "name": name})));
    ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 20, "height": 20, "parent": f, "props": {"name": "Dot", "fill": "#ff0000"}}));
    idv(&ex(s, "component.create", json!({"ids": [f]})))
}

fn shadowed_vector(s: &mut Session, parent: NodeId, name: &str) -> NodeId {
    let v = idv(&ex(s, "vector.create", json!({"path": "M0 0 L10 0 L10 10 Z", "parent": parent, "props": {"name": name, "fill": "#ff0000"}})));
    ex(s, "effect.add", json!({"ids": [v], "type": "dropShadow"}));
    v
}

fn host_with(s: &mut Session, name: &str, build: &dyn Fn(&mut Session, NodeId)) -> NodeId {
    let f = idv(&ex(s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 40, "name": name})));
    build(s, f);
    idv(&ex(s, "component.create", json!({"ids": [f]})))
}

fn svg_ids(def: &str) -> Vec<String> {
    def.split("id=\"").skip(1).filter_map(|rest| rest.split('"').next()).map(str::to_string).collect()
}

#[test]
fn inline_svg_ids_are_prefixed_by_the_layer_class_so_two_vectors_never_collide() {
    // covers: E6-4.8
    let mut s = Session::new();
    let comp = host_with(&mut s, "Marks", &|s, f| {
        shadowed_vector(s, f, "Mark A");
        shadowed_vector(s, f, "Mark B");
    });
    let def = definition(&mut s, comp);
    let ids = svg_ids(&def);
    assert!(ids.len() >= 2, "each shadow has a filter id:\n{def}");
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "ids are unique across the vectors: {ids:?}\n{def}");
    for class in ["mark-a", "mark-b"] {
        assert!(ids.iter().any(|id| id.starts_with(&format!("{class}-"))), "an id prefixed by {class}: {ids:?}");
    }
    for id in &ids {
        assert!(id.starts_with("mark-"), "{id} belongs to a layer: {ids:?}");
    }
}

fn colour_variable(s: &mut Session) {
    ex(s, "variable.createCollection", json!({"name": "Colors", "modes": ["Light"]}));
    ex(s, "variable.create", json!({"collection": "Colors", "name": "Colors/Brand", "type": "color", "values": {"Light": "#3366FF"}}));
}

#[test]
fn a_vector_bound_to_a_variable_is_noted_as_frozen() {
    // covers: E6-4.8
    let mut s = Session::new();
    colour_variable(&mut s);
    let comp = host_with(&mut s, "Marks", &|s, f| {
        let v = idv(&ex(s, "vector.create", json!({"path": "M0 0 L10 0 L10 10 Z", "parent": f, "props": {"name": "Mark", "fill": "#ff0000"}})));
        ex(s, "variable.bind", json!({"ids": [v], "field": "fill", "variable": "Colors/Brand"}));
        let w = idv(&ex(s, "vector.create", json!({"path": "M0 0 L10 0 L10 10 Z", "parent": f, "props": {"name": "Plain", "fill": "#00ff00"}})));
        ex(s, "node.set", json!({"id": w, "props": {"name": "Plain"}}));
    });
    let notes = notes_of(&component_code(&mut s, comp));
    let frozen: Vec<&Value> = notes.iter().filter(|n| n["code"] == "vector-token-frozen").collect();
    assert_eq!(frozen.len(), 1, "{notes:?}");
    assert!(frozen[0]["component"] == "Marks" && frozen[0]["layer"] == "Mark", "{notes:?}");
    assert!(frozen[0]["message"].as_str().unwrap_or_default().contains("Colors/Brand"), "{notes:?}");
}

#[test]
fn a_vector_with_an_image_fill_is_noted_and_embeds_no_image() {
    // covers: E6-4.7
    let mut s = Session::new();
    let image = image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 120, 200, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes.into_inner());
    let layer = idv(&ex(&mut s, "file.import", json!({"data": data, "name": "photo.png", "x": 0, "y": 600})));
    let fills = serde_json::to_value(&s.doc().unwrap().node(layer).unwrap().fills).unwrap();
    let comp = host_with(&mut s, "Marks", &|s, f| {
        let v = idv(&ex(s, "vector.create", json!({"path": "M0 0 L10 0 L10 10 Z", "parent": f, "props": {"name": "Photo"}})));
        ex(s, "node.set", json!({"id": v, "props": {"fills": fills.clone()}}));
    });
    let v = component_code(&mut s, comp);
    let note = note_with(&notes_of(&v), "image-fill").cloned();
    assert!(note.is_some_and(|n| n["layer"] == "Photo" && n["component"] == "Marks"), "{v}");
    let def = v["definition"].as_str().unwrap_or_default();
    assert!(!def.contains("data:image") && !def.contains("<image"), "no silent base64 in the file:\n{def}");
}

fn export_lit(s: &mut Session, extra: Value) -> Value {
    ex(s, "export.lit", extra)
}

fn issues_of(v: &Value, name: &str) -> Vec<String> {
    let entry = v["components"].as_array().and_then(|all| all.iter().find(|c| c["name"] == name));
    entry
        .and_then(|c| c["issues"].as_array())
        .map(|all| all.iter().filter_map(|i| i["code"].as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

#[test]
fn two_names_with_one_class_are_a_duplicate_and_the_export_is_not_ready() {
    // covers: E6-5.3
    let mut s = Session::new();
    plain_component(&mut s, "Icon 24", 0.0);
    plain_component(&mut s, "Icon24", 100.0);
    let v = export_lit(&mut s, json!({}));
    assert_eq!(v["ready"], false, "{v}");
    for name in ["Icon 24", "Icon24"] {
        assert!(issues_of(&v, name).contains(&"duplicate".to_string()), "{name}: {v}");
    }
}

#[test]
fn a_class_that_is_an_imported_name_is_reserved() {
    // covers: E6-5.3
    let mut s = Session::new();
    plain_component(&mut s, "Element", 0.0);
    let v = export_lit(&mut s, json!({"prefix": "lit-"}));
    assert_eq!(v["ready"], false, "{v}");
    assert!(issues_of(&v, "Element").contains(&"reserved".to_string()), "{v}");
}

fn tone_set(s: &mut Session, count: usize) -> NodeId {
    let mut variants = Vec::new();
    for k in 0..count {
        let name = format!("Tone=V{k:03}");
        let f = idv(&ex(s, "frame.create", json!({"x": k as f64 * 10.0, "y": 0, "width": 10, "height": 10, "name": name})));
        let fill = format!("#{:02x}{:02x}00", k % 256, (k / 256) % 256);
        ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 10, "height": 10, "parent": f, "props": {"name": "Bg", "fill": fill}}));
        variants.push(idv(&ex(s, "component.create", json!({"ids": [f]}))));
    }
    let set = idv(&ex(s, "component.combineVariants", json!({"ids": variants})));
    ex(s, "node.set", json!({"id": set, "props": {"name": "Tones"}}));
    set
}

#[test]
fn variants_up_to_the_cap_all_get_rules_and_beyond_it_a_note_says_so() {
    // covers: E6-1.2
    let mut s = Session::new();
    let set = tone_set(&mut s, 100);
    let v = component_code(&mut s, set);
    let def = v["definition"].as_str().unwrap_or_default();
    assert!(def.contains("[tone=\"V099\"]"), "the 100th variant has its rule:\n{def}");
    assert!(note_with(&notes_of(&v), "variants-truncated").is_none(), "{v}");

    let big = tone_set(&mut s, 300);
    let v = component_code(&mut s, big);
    let notes = notes_of(&v);
    let note = note_with(&notes, "variants-truncated");
    assert!(note.is_some_and(|n| n["component"] == "Tones" && n["layer"].is_null()), "{notes:?}");
    let def = v["definition"].as_str().unwrap_or_default();
    assert!(def.contains("[tone=\"V255\"]") && !def.contains("[tone=\"V256\"]"), "the cap is 256:\n{def}");
}

fn use_in(s: &mut Session, comp: NodeId, parent: NodeId) {
    ex(s, "instance.create", json!({"component": comp, "x": 0, "y": 0, "parent": parent}));
}

#[test]
fn a_component_imports_the_files_of_the_components_it_draws() {
    // covers: E6-1.1
    let mut s = Session::new();
    let icon = plain_component(&mut s, "Icon", 0.0);
    let hollow = idv(&ex(&mut s, "frame.create", json!({"x": 300, "y": 0, "width": 10, "height": 10, "name": "Hollow"})));
    let empty = idv(&ex(&mut s, "component.create", json!({"ids": [hollow]})));
    let card = host_with(&mut s, "Card", &|s, f| {
        use_in(s, icon, f);
        use_in(s, icon, f);
        use_in(s, empty, f);
    });
    let def = definition(&mut s, card);
    assert_eq!(def.matches("import \"./ui-icon.js\";").count(), 1, "one import per component:\n{def}");
    assert!(!def.contains("ui-hollow.js"), "an empty component has no file to import:\n{def}");
    let imports: Vec<&str> = def.lines().take_while(|l| !l.starts_with('@')).filter(|l| l.starts_with("import")).collect();
    assert_eq!(imports.last().copied(), Some("import \"./ui-icon.js\";"), "side-effect imports come after the Lit ones:\n{def}");
    assert!(!definition(&mut s, icon).contains("import \"./"), "a leaf imports nothing");
}

#[test]
fn a_property_named_like_a_member_of_the_element_gets_a_suffix() {
    // covers: E6-1.2
    let mut s = Session::new();
    for (name, expected) in [
        ("Scroll", "scrollVariant"),
        ("Constructor", "constructorVariant"),
        ("Tabindex", "tabindexVariant"),
        ("Sync Focus", "syncFocusVariant"),
        ("Aria Label", "ariaLabelVariant"),
        ("Query Selector", "querySelectorVariant"),
    ] {
        let mut variants = Vec::new();
        for (k, value) in ["A", "B"].into_iter().enumerate() {
            let f = idv(&ex(
                &mut s,
                "frame.create",
                json!({"x": k as f64 * 150.0, "y": 0, "width": 10, "height": 10, "name": format!("{name}={value}")}),
            ));
            ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 10, "height": 10, "parent": f, "props": {"name": "Bg"}}));
            variants.push(idv(&ex(&mut s, "component.create", json!({"ids": [f]}))));
        }
        let set = idv(&ex(&mut s, "component.combineVariants", json!({"ids": variants})));
        ex(&mut s, "node.set", json!({"id": set, "props": {"name": format!("Host {name}")}}));
        let def = definition(&mut s, set);
        assert!(def.contains(&format!(" {expected}: \"A\" | \"B\"")), "{name} → {expected}:\n{def}");
    }
}

#[test]
fn a_spinner_without_a_loading_state_or_boolean_is_noted() {
    // covers: E6-4.5
    let mut s = Session::new();
    let bare = host_with(&mut s, "Bare", &|s, f| {
        ex(s, "shape.ellipse", json!({"x": 0, "y": 0, "width": 10, "height": 10, "parent": f, "props": {"name": "Spinner"}}));
    });
    let notes = notes_of(&component_code(&mut s, bare));
    let note = note_with(&notes, "loading-missing");
    assert!(note.is_some_and(|n| n["component"] == "Bare" && n["layer"] == "Spinner"), "{notes:?}");

    let mut variants = Vec::new();
    for (k, state) in ["Default", "Loading"].into_iter().enumerate() {
        let f = idv(&ex(&mut s, "frame.create", json!({"x": k as f64 * 150.0, "y": 0, "width": 10, "height": 10, "name": format!("State={state}")})));
        ex(&mut s, "shape.ellipse", json!({"x": 0, "y": 0, "width": 10, "height": 10, "parent": f, "props": {"name": "Spinner"}}));
        variants.push(idv(&ex(&mut s, "component.create", json!({"ids": [f]}))));
    }
    let set = idv(&ex(&mut s, "component.combineVariants", json!({"ids": variants})));
    ex(&mut s, "node.set", json!({"id": set, "props": {"name": "Busy"}}));
    let notes = notes_of(&component_code(&mut s, set));
    assert!(note_with(&notes, "loading-missing").is_none(), "a Loading state is enough: {notes:?}");
}

struct Folder(std::path::PathBuf);

impl Folder {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-lit-review-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_comparison_that_would_render_too_many_pixels_is_refused_before_rendering() {
    // covers: E6-5.1
    let mut s = Session::new();
    let dir = Folder::new();
    let png = dir.0.join("one.png");
    let pix = figcraft_render::Pixmap::new(1, 1).unwrap();
    std::fs::write(&png, figcraft_render::encode_png(&pix).unwrap()).unwrap();
    let image = png.to_string_lossy().into_owned();
    let big = idv(&ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 20_000, "height": 4_000, "props": {"name": "Huge"}})));
    let Err(crate::EngineError::BadParams { msg, .. }) = s.execute("export.litCompare", &json!({"id": big, "image": image})) else {
        panic!("a 80 megapixel render is refused");
    };
    assert!(msg.contains("megapixels"), "{msg}");
    let small = idv(&ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 10, "height": 10, "props": {"name": "Small"}})));
    let ok = ex(&mut s, "export.litCompare", json!({"id": small, "image": image, "scale": 2}));
    assert_eq!(ok["sizeMismatch"], true, "{ok}");
}
