//! Lit component definitions: `inspect.componentCode {language: "lit"}` writes a `LitElement`
//! web component (`@customElement`, `@property` fields, `static styles = css`, `render()`), and
//! `inspect.code {language: "lit"}` on an instance writes the custom element with its values.

use crate::Session;
use crate::codegen::CodeLang;
use crate::components::tests::{ex, idv};
use figcraft_doc::*;
use figcraft_plugin::bundle::sha256_hex;
use serde_json::{Value, json};

const HOSTILE: &str = "a`b${alert(1)}\\c";

fn component_code(s: &mut Session, id: NodeId, lang: &str) -> Value {
    ex(s, "inspect.componentCode", json!({"id": id, "language": lang}))
}

fn code(s: &mut Session, id: NodeId, lang: &str) -> String {
    ex(s, "inspect.code", json!({"id": id, "language": lang}))["code"].as_str().unwrap_or_default().to_string()
}

fn definition(s: &mut Session, id: NodeId) -> String {
    component_code(s, id, "lit")["definition"].as_str().unwrap_or_default().to_string()
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").replace("> <", "><")
}

fn decorator_of(text: &str, ident: &str) -> String {
    let compacted = compact(text);
    let Some(member) = compacted.find(&format!(") {ident}")) else { return String::new() };
    let start = compacted.get(..member).and_then(|head| head.rfind("@property(")).unwrap_or(member);
    compacted.get(start..=member).unwrap_or_default().to_string()
}

fn property<'a>(v: &'a Value, name: &str) -> &'a Value {
    v["properties"].as_array().and_then(|all| all.iter().find(|p| p["name"] == name)).unwrap_or(&Value::Null)
}

fn has_unescaped(text: &str, needle: &str) -> bool {
    text.match_indices(needle).any(|(at, _)| text.get(..at).unwrap_or_default().chars().rev().take_while(|c| *c == '\\').count() % 2 == 0)
}

fn string_after(text: &str, markers: &[&str]) -> Option<String> {
    let (at, marker) = markers.iter().find_map(|m| text.find(m).map(|at| (at, *m)))?;
    let mut chars = text.get(at + marker.len()..)?.chars();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                '0' => out.push('\0'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                'x' => {
                    let hex: String = chars.by_ref().take(2).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                other => out.push(other),
            },
            other => out.push(other),
        }
    }
    None
}

/// A "Button" set: `Style=Primary|Secondary` (Secondary has a blue background), text properties
/// "Label" ("Click") and "Title" ("Hi"), and the boolean "Show icon". Returns (set, default variant).
fn button_set(s: &mut Session) -> (NodeId, NodeId) {
    let mut variants = Vec::new();
    for (k, (style, color)) in [("Primary", "#ff0000"), ("Secondary", "#0000ff")].iter().enumerate() {
        let f = idv(&ex(s, "frame.create", json!({"x": k as f64 * 150.0, "y": 300, "width": 100, "height": 40, "name": format!("Style={style}")})));
        ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": {"name": "Bg", "fill": color}}));
        ex(s, "text.create", json!({"text": "Click", "x": 10, "y": 10, "parent": f, "props": {"name": "Label"}}));
        ex(s, "text.create", json!({"text": "Hi", "x": 10, "y": 20, "parent": f, "props": {"name": "Heading"}}));
        ex(s, "shape.ellipse", json!({"x": 80, "y": 10, "width": 10, "height": 10, "parent": f, "props": {"name": "Icon"}}));
        variants.push(idv(&ex(s, "component.create", json!({"ids": [f]}))));
    }
    let set = idv(&ex(s, "component.combineVariants", json!({"ids": variants.clone()})));
    ex(s, "node.set", json!({"id": set, "props": {"name": "Button"}}));
    let kids = s.doc().unwrap().node(variants[0]).unwrap().children.clone();
    let layer = |s: &Session, name: &str| kids.iter().copied().find(|k| s.doc().unwrap().node(*k).unwrap().name == name).unwrap();
    let (label, heading, icon) = (layer(s, "Label"), layer(s, "Heading"), layer(s, "Icon"));
    ex(s, "component.addProperty", json!({"id": set, "name": "Label", "type": "text", "layer": label}));
    ex(s, "component.addProperty", json!({"id": set, "name": "Title", "type": "text", "layer": heading}));
    ex(s, "component.addProperty", json!({"id": set, "name": "Show icon", "type": "boolean", "layer": icon, "default": true}));
    (set, variants[0])
}

/// A set named `name` with one variant property `prop` taking `values`, each variant a frame with
/// a background of its own colour.
fn variant_set(s: &mut Session, name: &str, prop: &str, values: &[&str]) -> NodeId {
    let mut variants = Vec::new();
    for (k, value) in values.iter().enumerate() {
        let f = idv(&ex(s, "frame.create", json!({"x": k as f64 * 150.0, "y": 0, "width": 100, "height": 40, "name": format!("{prop}={value}")})));
        let fill = if k == 0 { "#ff0000" } else { "#0000ff" };
        ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": {"name": "Bg", "fill": fill}}));
        variants.push(idv(&ex(s, "component.create", json!({"ids": [f]}))));
    }
    let set = idv(&ex(s, "component.combineVariants", json!({"ids": variants})));
    ex(s, "node.set", json!({"id": set, "props": {"name": name}}));
    set
}

#[test]
fn a_component_becomes_a_lit_element() {
    // covers: E6-1.1
    let mut s = Session::new();
    let (set, _) = button_set(&mut s);
    let v = component_code(&mut s, set, "lit");
    assert_eq!(v["language"], "lit");
    assert_eq!(v["tag"], "ui-button");
    assert_eq!(v["className"], "UiButton");
    assert!(v["notes"].is_array(), "{v}");
    let def = v["definition"].as_str().unwrap_or_default();
    let text = compact(def);
    for want in [
        "import { LitElement, css, html, nothing } from \"lit\";",
        "import { customElement, property } from \"lit/decorators.js\";",
        "@customElement(\"ui-button\")",
        "export class UiButton extends LitElement {",
        "declare global {",
        "HTMLElementTagNameMap",
        "\"ui-button\": UiButton;",
        "static styles = css`",
        "render() { return html`",
        "part=\"root\"",
        ":host { display: block; }",
    ] {
        assert!(text.contains(want), "Lit has {want:?}:\n{def}");
    }
    assert_eq!(code(&mut s, set, "lit"), def, "a component's code is its definition");
}

#[test]
fn variant_properties_are_reflected_unions_without_data_attributes() {
    // covers: E6-1.2
    let mut s = Session::new();
    let badge = variant_set(&mut s, "Badge", "Tone", &["Soft", "Loud"]);
    let def = definition(&mut s, badge);
    assert!(compact(&def).contains("@property({ reflect: true }) tone: \"Soft\" | \"Loud\" = \"Soft\";"), "{def}");
    assert!(!def.contains("data-"), "no data attribute:\n{def}");
    let (set, _) = button_set(&mut s);
    let def = definition(&mut s, set);
    assert!(compact(&def).contains("styleVariant: \"Primary\" | \"Secondary\" = \"Primary\";"), "{def}");
    assert!(!def.contains("data-"), "no data attribute:\n{def}");
}

#[test]
fn names_that_collide_with_html_element_members_get_a_suffix() {
    // covers: E6-1.3
    let mut s = Session::new();
    let (set, _) = button_set(&mut s);
    let v = component_code(&mut s, set, "lit");
    let def = v["definition"].as_str().unwrap_or_default();
    let text = compact(def);
    assert!(text.contains(") styleVariant: "), "a variant property takes `Variant`:\n{def}");
    assert!(text.contains(") titleValue"), "any other takes `Value`:\n{def}");
    assert!(decorator_of(def, "styleVariant").contains("attribute: \"style-variant\""), "{def}");
    assert!(decorator_of(def, "titleValue").contains("attribute: \"title-value\""), "{def}");
    for bad in [") style:", ") style =", ") title:", ") title =", " style: \"Primary\"", "title = \"Hi\""] {
        assert!(!text.contains(bad), "{bad:?} would shadow an HTMLElement member:\n{def}");
    }
    assert_eq!(property(&v, "Style")["attribute"], "style-variant");
    assert_eq!(property(&v, "Title")["attribute"], "title-value");
    assert_eq!(property(&v, "Label")["attribute"], "label", "a free name is kept");
    assert!(text.contains(") label = \"Click\";") || text.contains(") label: string = \"Click\";"), "{def}");
    assert_eq!(v["properties"][0]["param"], "style", "describe() keeps its own parameter names");

    let f = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 100, "name": "Card"})));
    let id_text = idv(&ex(&mut s, "text.create", json!({"text": "x", "x": 0, "y": 0, "parent": f, "props": {"name": "Ident"}})));
    let class_text = idv(&ex(&mut s, "text.create", json!({"text": "y", "x": 0, "y": 20, "parent": f, "props": {"name": "Klass"}})));
    let dot = idv(&ex(&mut s, "shape.ellipse", json!({"x": 80, "y": 0, "width": 10, "height": 10, "parent": f, "props": {"name": "Dot"}})));
    let card = idv(&ex(&mut s, "component.create", json!({"ids": [f]})));
    ex(&mut s, "component.addProperty", json!({"id": card, "name": "Id", "type": "text", "layer": id_text}));
    ex(&mut s, "component.addProperty", json!({"id": card, "name": "Class name", "type": "text", "layer": class_text}));
    ex(&mut s, "component.addProperty", json!({"id": card, "name": "Hidden", "type": "boolean", "layer": dot, "default": false}));
    let def = definition(&mut s, card);
    let text = compact(&def);
    for (member, attribute) in [("idValue", "id-value"), ("classNameValue", "class-name-value"), ("hiddenValue", "hidden-value")] {
        assert!(text.contains(&format!(") {member}")), "{member}:\n{def}");
        assert!(decorator_of(&def, member).contains(&format!("attribute: \"{attribute}\"")), "{member}:\n{def}");
    }
    for bad in [") id ", ") id:", ") className ", ") className:", ") hidden ", ") hidden:"] {
        assert!(!text.contains(bad), "{bad:?} would shadow an HTMLElement member:\n{def}");
    }
}

#[test]
fn multi_word_properties_get_a_kebab_attribute_and_booleans_read_false() {
    // covers: E6-1.4
    let mut s = Session::new();
    let (set, _) = button_set(&mut s);
    let v = component_code(&mut s, set, "lit");
    let def = v["definition"].as_str().unwrap_or_default();
    let decorator = decorator_of(def, "showIcon");
    assert!(decorator.contains("attribute: \"show-icon\""), "{decorator}\n{def}");
    assert!(decorator.contains("fromAttribute") && decorator.contains("\"false\""), "show-icon=\"false\" must turn it off: {decorator}");
    let text = compact(def);
    assert!(text.contains(") showIcon = true;") || text.contains(") showIcon: boolean = true;"), "{def}");
    assert_eq!(property(&v, "Show icon")["attribute"], "show-icon");
    assert_eq!(property(&v, "Show icon")["reflect"], false);
}

#[test]
fn layers_bind_to_properties_and_slots_keep_their_default_content() {
    // covers: E6-1.5
    let mut s = Session::new();
    let (set, _) = button_set(&mut s);
    let text = compact(&definition(&mut s, set));
    assert!(text.contains("${this.label}"), "{text}");
    assert!(text.contains("${this.titleValue}"), "{text}");
    assert!(text.contains("this.showIcon ?") && text.contains(": nothing"), "a boolean-bound layer is conditional:\n{text}");
    assert!(text.contains("class=\"icon\""), "{text}");

    let frame = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 200, "height": 100, "name": "Shell"})));
    let body = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 200, "height": 100, "parent": frame, "name": "Body"})));
    ex(&mut s, "text.create", json!({"text": "Empty", "x": 0, "y": 0, "parent": body, "props": {"name": "Hint"}}));
    let shell = idv(&ex(&mut s, "component.create", json!({"ids": [frame]})));
    ex(&mut s, "component.makeSlot", json!({"id": body, "name": "Content"}));
    let def = definition(&mut s, shell);
    let text = compact(&def);
    let from = text.find("<slot name=\"content\">").unwrap_or(usize::MAX);
    let to = text.find("</slot>").unwrap_or(0);
    assert!(from < to, "a named slot:\n{def}");
    assert!(text.get(from..to).unwrap_or_default().contains("Empty"), "the slot's content is its default:\n{def}");

    let icon = |s: &mut Session, name: &str, x: f64| {
        let f = idv(&ex(s, "frame.create", json!({"x": x, "y": 400, "width": 24, "height": 24, "name": name})));
        ex(s, "shape.star", json!({"x": 0, "y": 0, "width": 24, "height": 24, "parent": f}));
        idv(&ex(s, "component.create", json!({"ids": [f]})))
    };
    let heart = icon(&mut s, "Heart", 0.0);
    let f = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 120, "height": 40, "name": "Chip"})));
    let nested = idv(&ex(&mut s, "instance.create", json!({"component": heart, "x": 8, "y": 8, "parent": f})));
    let chip = idv(&ex(&mut s, "component.create", json!({"ids": [f]})));
    ex(&mut s, "component.addProperty", json!({"id": chip, "name": "Icon", "type": "instanceSwap", "default": heart, "layer": nested}));
    let def = definition(&mut s, chip);
    assert!(compact(&def).contains("<slot name=\"icon\"><ui-heart></ui-heart></slot>"), "an instance swap is a slot holding the default:\n{def}");
}

#[test]
fn variant_differences_are_host_attribute_selectors() {
    // covers: E6-1.6
    let mut s = Session::new();
    let (set, _) = button_set(&mut s);
    let def = definition(&mut s, set);
    let text = compact(&def);
    let rule = ":host([style-variant=\"Secondary\"]) .bg {";
    let at = text.find(rule).unwrap_or_else(|| panic!("{rule} in:\n{def}"));
    let body = text.get(at + rule.len()..).and_then(|tail| tail.split('}').next()).unwrap_or_default().trim();
    assert_eq!(body, "background: #0000FF;", "only what differs:\n{def}");
    assert!(!text.contains("[data-"), "{def}");

    let mut variants = Vec::new();
    for (k, (first, second, fill)) in [("Small", "Big", "#0000FF"), ("Large", "Small", "#FF0000")].iter().enumerate() {
        let f = idv(&ex(
            &mut s,
            "frame.create",
            json!({"x": k as f64 * 150.0, "y": 0, "width": 100, "height": 40, "name": format!("Size={first}, size={second}"), "props": {"fill": fill}}),
        ));
        variants.push(idv(&ex(&mut s, "component.create", json!({"ids": [f]}))));
    }
    let chip = idv(&ex(&mut s, "component.combineVariants", json!({"ids": variants})));
    ex(&mut s, "node.set", json!({"id": chip, "props": {"name": "Chip"}}));
    let v = component_code(&mut s, chip, "lit");
    let upper = property(&v, "Size")["attribute"].as_str().unwrap_or_default().to_string();
    let lower = property(&v, "size")["attribute"].as_str().unwrap_or_default().to_string();
    assert!(!upper.is_empty() && !lower.is_empty(), "{v}");
    assert_ne!(upper.to_lowercase(), lower.to_lowercase(), "HTML attribute names ignore case: {upper} / {lower}");
    let text = compact(v["definition"].as_str().unwrap_or_default());
    let both = [format!(":host([{upper}=\"Large\"][{lower}=\"Small\"])"), format!(":host([{lower}=\"Small\"][{upper}=\"Large\"])")];
    assert!(both.iter().any(|sel| text.contains(sel)), "{both:?} in:\n{text}");
    assert!(text.contains("#FF0000"), "{text}");
}

#[test]
fn hostile_text_is_escaped_in_the_template_literals() {
    // covers: E6-1.7
    let mut s = Session::new();
    let mut variants = Vec::new();
    for (k, tone) in ["Soft", "Lo`${alert(1)}\\w"].iter().enumerate() {
        let f = idv(&ex(&mut s, "frame.create", json!({"x": k as f64 * 150.0, "y": 0, "width": 100, "height": 40, "name": format!("Tone={tone}")})));
        ex(
            &mut s,
            "shape.rectangle",
            json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": {"name": "Bg`${alert(1)}\\x", "fill": "#ff0000"}}),
        );
        ex(&mut s, "text.create", json!({"text": HOSTILE, "x": 10, "y": 10, "parent": f, "props": {"name": "Label"}}));
        variants.push(idv(&ex(&mut s, "component.create", json!({"ids": [f]}))));
    }
    let set = idv(&ex(&mut s, "component.combineVariants", json!({"ids": variants.clone()})));
    ex(&mut s, "node.set", json!({"id": set, "props": {"name": "Badge"}}));
    let label =
        s.doc().unwrap().node(variants[0]).unwrap().children.iter().copied().find(|k| s.doc().unwrap().node(*k).unwrap().name == "Label").unwrap();
    ex(&mut s, "component.addProperty", json!({"id": set, "name": "Label", "type": "text", "layer": label}));
    let def = definition(&mut s, set);
    assert!(!has_unescaped(&def, "${alert(1)}"), "an interpolation would run:\n{def}");
    assert!(!def.contains("a`b") && !def.contains("Lo`"), "a raw backtick:\n{def}");
    assert!(def.contains("\\\\c"), "the backslash is doubled:\n{def}");
    assert_eq!(string_after(&def, &["label = \"", "label: string = \""]).as_deref(), Some(HOSTILE), "readable once decoded:\n{def}");
    assert_eq!(string_after(&def, &["\"Soft\" | \""]).as_deref(), Some("Lo`${alert(1)}\\w"), "{def}");
}

#[test]
fn instances_are_custom_elements_and_the_language_round_trips() {
    // covers: E6-1.8
    let mut s = Session::new();
    let (set, _) = button_set(&mut s);
    let i = idv(&ex(&mut s, "instance.create", json!({"component": set, "x": 400, "y": 0})));
    assert!(code(&mut s, i, "lit").starts_with("<ui-button></ui-button>"), "defaults aren't passed:\n{}", code(&mut s, i, "lit"));
    ex(&mut s, "instance.setProperties", json!({"ids": [i], "properties": {"Style": "Secondary", "Label": "Buy"}}));
    let out = code(&mut s, i, "lit");
    assert!(out.starts_with("<ui-button style-variant=\"Secondary\" label=\"Buy\"></ui-button>"), "{out}");
    assert!(out.contains("export class UiButton extends LitElement {"), "the definition follows the call:\n{out}");

    for cmd in ["inspect.code", "inspect.componentCode"] {
        let err = s.execute(cmd, &json!({"id": set, "language": "cobol"}));
        assert!(err.is_err(), "{cmd}: an unknown language is refused");
    }
    for id in ["lit", "Lit", "LIT"] {
        let lang = CodeLang::from_id(id);
        assert_eq!(lang.map(CodeLang::id), Some("lit"), "{id}");
        assert!(lang.is_some_and(CodeLang::is_web));
    }
    assert_eq!(CodeLang::from_id("lit").map(CodeLang::label), Some("Lit"));
    assert_eq!(CodeLang::ALL.last().map(|l| l.id()), Some("lit"), "added at the end of the list");
}

#[test]
fn a_component_connected_to_react_code_keeps_its_lit_definition() {
    // covers: E6-1.9
    let mut s = Session::new();
    let (set, _) = button_set(&mut s);
    ex(&mut s, "codeConnect.set", json!({"id": set, "importPath": "@acme/ui", "template": "<Button />"}));
    assert!(code(&mut s, set, "react").contains("@acme/ui"), "the link exists");
    let v = component_code(&mut s, set, "lit");
    assert!(v["connected"].is_null(), "{v}");
    assert!(v["definition"].as_str().unwrap_or_default().contains("export class UiButton extends LitElement {"), "{v}");
    assert!(code(&mut s, set, "lit").contains("@customElement(\"ui-button\")"));
}

const REACT_BUTTON: &str = r###"import "./button.css";

export interface ButtonProps {
  style?: "Primary" | "Secondary";
  label?: string;
  showIcon?: boolean;
  title?: string;
}

export function Button({
  style = "Primary",
  label = "Click",
  showIcon = true,
  title = "Hi",
}: ButtonProps) {
  return (
    <div className="button" data-style={style}>
      <div className="bg"></div>
      <p className="label">{label}</p>
      <p className="heading">{title}</p>
      {showIcon && (
        <div className="icon"></div>
      )}
    </div>
  );
}

/* button.css */
.button {
  position: relative;
  width: 100px;
  height: 40px;
  background: #FFFFFF;
}
.bg {
  position: absolute;
  left: 0px;
  top: 0px;
  width: 100px;
  height: 40px;
  background: #FF0000;
}
.label {
  position: absolute;
  left: 10px;
  top: 10px;
  color: #000000;
  font-family: "Inter";
  font-size: 12px;
  font-weight: 400;
}
.heading {
  position: absolute;
  left: 10px;
  top: 20px;
  color: #000000;
  font-family: "Inter";
  font-size: 12px;
  font-weight: 400;
}
.icon {
  position: absolute;
  left: 80px;
  top: 10px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #D9D9D9;
}
.button[data-style="Secondary"] .bg {
  background: #0000FF;
}
"###;

const VUE_BUTTON: &str = r###"<script setup lang="ts">

const __props = withDefaults(
  defineProps<{
    style?: "Primary" | "Secondary";
    label?: string;
    showIcon?: boolean;
    title?: string;
  }>(),
  {
    style: "Primary",
    label: "Click",
    showIcon: true,
    title: "Hi",
  },
);

</script>

<template>
    <div class="button" :data-style="style">
      <div class="bg"></div>
      <p class="label">{{ label }}</p>
      <p class="heading">{{ title }}</p>
      <div class="icon" v-if="showIcon"></div>
    </div>
</template>

<style scoped>
.button {
  position: relative;
  width: 100px;
  height: 40px;
  background: #FFFFFF;
}
.bg {
  position: absolute;
  left: 0px;
  top: 0px;
  width: 100px;
  height: 40px;
  background: #FF0000;
}
.label {
  position: absolute;
  left: 10px;
  top: 10px;
  color: #000000;
  font-family: "Inter";
  font-size: 12px;
  font-weight: 400;
}
.heading {
  position: absolute;
  left: 10px;
  top: 20px;
  color: #000000;
  font-family: "Inter";
  font-size: 12px;
  font-weight: 400;
}
.icon {
  position: absolute;
  left: 80px;
  top: 10px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #D9D9D9;
}
.button[data-style="Secondary"] .bg {
  background: #0000FF;
}
</style>
"###;

#[test]
fn the_other_languages_are_unchanged() {
    // covers: E6-1.10
    let mut s = Session::new();
    let (set, _) = button_set(&mut s);
    assert_eq!(component_code(&mut s, set, "react")["definition"], REACT_BUTTON);
    assert_eq!(code(&mut s, set, "react"), REACT_BUTTON);
    assert_eq!(component_code(&mut s, set, "vue")["definition"], VUE_BUTTON);
    assert_eq!(code(&mut s, set, "vue"), VUE_BUTTON);
    for (lang, want) in [
        ("html", "4f22106ab567445ca9ae3e2fecf3a51f962372c0ded1ed617d27780bfbb71943"),
        ("css", "1f0e88adcd94017be134c501f86aaf6eefe6624c14e0f84048ca3a89387dac8e"),
        ("tailwind", "e51fddddcdda77a2b41b3834376af4e91a17024362ceda9429f13151142e7658"),
        ("leptos", "c4b620e3afa9c2ab140a9569fb460d4ecc4cb309e5f0f33d35606e7fcd6c33e3"),
        ("swiftui", "d55f58db3d31ba86bd718f638671975436ab442a1ea30d92b185c59ea6f7669e"),
        ("compose", "e763d0003268fc7f9beeee141237fc056544e1abeae70a9498b8490beff28fb2"),
        ("arkui", "c7b4ea209e9ca838bc28bab98ba7497de0dd8bacc40e8be6d82bdc6c924d1498"),
    ] {
        assert_eq!(sha256_hex(code(&mut s, set, lang).as_bytes()), want, "{lang} output of the set changed");
    }
    let i = idv(&ex(&mut s, "instance.create", json!({"component": set, "x": 400, "y": 0})));
    ex(&mut s, "instance.setProperties", json!({"ids": [i], "properties": {"Style": "Secondary", "Label": "Buy"}}));
    for (lang, usage) in [
        ("react", "<Button style=\"Secondary\" label=\"Buy\" />\n"),
        ("vue", "<Button style=\"Secondary\" label=\"Buy\" />\n"),
        ("leptos", "<Button style=ButtonStyle::Secondary label=\"Buy\" />\n"),
        ("swiftui", "Button(style: .secondary, label: \"Buy\")\n"),
        ("compose", "Button(style = ButtonStyle.Secondary, label = \"Buy\")\n"),
        ("arkui", "Button({ style: ButtonStyle.Secondary, label: \"Buy\" })\n"),
    ] {
        assert_eq!(component_code(&mut s, i, lang)["usage"], usage, "{lang}");
        assert!(code(&mut s, i, lang).starts_with(usage), "{lang}");
    }
    for (lang, want) in [
        ("html", "5ec6c04b6c137b1cc1986ee2d0ce72bd34b084d2da22f1174c8a9d02f8f852a8"),
        ("react", "b102da905ccc02617c37f59c528790d3b641097a2d20385268ad1d652eb88884"),
        ("vue", "f8130ee1f3f6fd726a93bb6daa69d10c81b26ff458e77191d262f6e981981c1c"),
        ("leptos", "f898a94f2d2c008f8e03b35b0c61b435f3796461c787f79a4f52ffa4e9d00d72"),
        ("swiftui", "a9671a050b8ae5f46ba4c60c97dfe7bbcca52c1162a0d9f4ee27b6dea0fb87d9"),
        ("compose", "e761d011f417ec738826b37a01940e16e99415bfa13995af252715ac01b43b43"),
        ("arkui", "478f5e15f323c6cf464aff9ef8fa8098274301bcb42d55b4b715b8baef372d8b"),
    ] {
        assert_eq!(sha256_hex(code(&mut s, i, lang).as_bytes()), want, "{lang} output of the instance changed");
    }
}
