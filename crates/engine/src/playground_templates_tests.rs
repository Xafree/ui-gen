//! Playground templates written with the library's tags (E7-3.1 to E7-3.6): `compose` resolves the
//! components a template asks for against the tags of an export plan, marks the ones the plan lacks,
//! passes only the attributes a component knows, builds the Components page from the plan and
//! escapes everything it writes. The plans are built by hand; the names a template asks for are read
//! from `Composed::missing` of a composition against an empty plan.

use std::collections::{BTreeMap, BTreeSet};

use figcraft_doc::NodeId;

use crate::lit::StateRecipe;
use crate::lit_export::{ExportPlan, Issue, PlannedComponent, Status};
use crate::playground::{Template, compose};

const TEMPLATE_NAMES: [&str; 6] = ["components", "dashboard", "mail", "chat", "finance", "crm"];
const MARKER_TEXT: &str = "Composant absent de la lib : ";
const NATIVE_ELEMENTS: [&str; 4] = ["button", "input", "select", "textarea"];
const PROPERTY_KEYS: [&str; 40] = [
    "variant",
    "size",
    "color",
    "kind",
    "type",
    "appearance",
    "tone",
    "intent",
    "shape",
    "state",
    "disabled",
    "checked",
    "selected",
    "active",
    "label",
    "value",
    "name",
    "placeholder",
    "icon",
    "orientation",
    "density",
    "elevation",
    "align",
    "heading",
    "status",
    "level",
    "trend",
    "compact",
    "loading",
    "required",
    "invalid",
    "error",
    "open",
    "pressed",
    "mode",
    "layout",
    "position",
    "direction",
    "accent",
    "outlined",
];

fn template(name: &str) -> Template {
    match name {
        "components" => Template::Components,
        "dashboard" => Template::Dashboard,
        "mail" => Template::Mail,
        "chat" => Template::Chat,
        "finance" => Template::Finance,
        "crm" => Template::Crm,
        other => panic!("no template named {other}"),
    }
}

fn norm(name: &str) -> String {
    name.chars().filter(|c| !c.is_whitespace() && *c != '-').flat_map(char::to_lowercase).collect()
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect()
}

fn recipe(id: u64, props: &[(&str, &str)], attrs: &[(&str, &str)]) -> StateRecipe {
    StateRecipe { id: NodeId(id), props: map(props), attrs: map(attrs), pseudo: None }
}

fn planned(id: u64, name: &str, tag: &str, status: Status, states: Vec<StateRecipe>) -> PlannedComponent {
    let issues = match status {
        Status::Ok | Status::Dependency => vec![],
        Status::Empty => vec![Issue { code: "empty", message: format!("\"{name}\" has no layer") }],
        Status::Invalid => vec![Issue { code: "noHyphen", message: format!("\"{name}\" gives an element name without a hyphen") }],
    };
    let exportable = status == Status::Ok;
    PlannedComponent {
        id: NodeId(id),
        name: name.to_string(),
        tag: (status != Status::Invalid).then(|| tag.to_string()),
        class_name: exportable.then(|| "Generated".to_string()),
        file: exportable.then(|| format!("components/{tag}.ts")),
        status,
        issues,
        states,
    }
}

fn ok(id: u64, name: &str, states: Vec<StateRecipe>) -> PlannedComponent {
    planned(id, name, &format!("ui-{}", slug(name)), Status::Ok, states)
}

fn plan_of(components: Vec<PlannedComponent>) -> ExportPlan {
    ExportPlan { components, files: vec![], notes: vec![] }
}

fn plan_named(names: &[&str]) -> ExportPlan {
    plan_of(names.iter().zip(1u64..).map(|(name, id)| ok(id, name, vec![recipe(id, &[], &[])])).collect())
}

fn plan_with_keys(names: &[String], keys: &[&str]) -> ExportPlan {
    let pairs: Vec<(&str, &str)> = keys.iter().map(|k| (*k, "v")).collect();
    plan_of(names.iter().zip(1u64..).map(|(name, id)| ok(id, name, vec![recipe(id, &pairs, &pairs)])).collect())
}

fn requested(name: &str) -> Vec<String> {
    compose(template(name), &plan_of(vec![])).missing
}

fn requested_by_the_pages() -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for page in &TEMPLATE_NAMES[1..] {
        for name in requested(page) {
            if !names.iter().any(|n| norm(n) == norm(&name)) {
                names.push(name);
            }
        }
    }
    if !names.iter().any(|n| norm(n) == "button") {
        names.push("Button".to_string());
    }
    names
}

fn full_plan() -> ExportPlan {
    let names = requested_by_the_pages();
    plan_of(names.iter().zip(1u64..).map(|(name, id)| ok(id, name, vec![recipe(id, &[], &[])])).collect())
}

struct Tag {
    name: String,
    attrs: Vec<(String, Option<String>)>,
    closing: bool,
    at: usize,
    end: usize,
}

fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let semi = after.find(';').unwrap_or_else(|| panic!("a bare & in {text:?}"));
        let entity = &after[..semi];
        let decoded = match entity {
            "quot" => Some('"'),
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        out.push(decoded.unwrap_or_else(|| panic!("a bare & or an unknown entity &{entity}; in {text:?}")));
        rest = &after[semi + 1..];
    }
    out.push_str(rest);
    out
}

fn tags(html: &str) -> Vec<Tag> {
    let b = html.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b.get(i) != Some(&b'<') {
            i += 1;
            continue;
        }
        let at = i;
        if html[i..].starts_with("<!--") {
            let close = html[i..].find("-->").unwrap_or_else(|| panic!("an unclosed comment at {at}"));
            i += close + 3;
            out.push(Tag { name: "!--".into(), attrs: vec![], closing: false, at, end: i });
            continue;
        }
        i += 1;
        let closing = b.get(i) == Some(&b'/');
        if closing {
            i += 1;
        }
        let start = i;
        while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'-') {
            i += 1;
        }
        assert!(i > start && b[start].is_ascii_alphabetic(), "a stray < at {at}: {:?}", &html[at..html.len().min(at + 40)]);
        let name = html[start..i].to_ascii_lowercase();
        let mut attrs = Vec::new();
        loop {
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            match b.get(i) {
                None => panic!("the <{name}> at {at} is never closed by >"),
                Some(b'>') => {
                    i += 1;
                    break;
                }
                Some(b'/') => {
                    i += 1;
                    continue;
                }
                Some(_) => {}
            }
            let s = i;
            while i < b.len() && !(b[i].is_ascii_whitespace() || matches!(b[i], b'=' | b'>' | b'/')) {
                i += 1;
            }
            assert!(i > s, "an attribute without a name in the <{name}> at {at}");
            let attr = html[s..i].to_ascii_lowercase();
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if b.get(i) == Some(&b'=') {
                i += 1;
                while i < b.len() && b[i].is_ascii_whitespace() {
                    i += 1;
                }
                let quote = match b.get(i) {
                    Some(q @ (b'"' | b'\'')) => *q,
                    _ => panic!("the attribute {attr} of the <{name}> at {at} is not quoted"),
                };
                i += 1;
                let from = i;
                while i < b.len() && b[i] != quote {
                    i += 1;
                }
                assert!(i < b.len(), "the attribute {attr} of the <{name}> at {at} is never closed");
                let raw = &html[from..i];
                assert!(!raw.contains('<'), "a raw < in the attribute {attr} of the <{name}> at {at}: {raw:?}");
                attrs.push((attr, Some(unescape(raw))));
                i += 1;
            } else {
                attrs.push((attr, None));
            }
        }
        out.push(Tag { name, attrs, closing, at, end: i });
    }
    out
}

fn text_of(html: &str) -> String {
    let mut out = String::new();
    let mut pos = 0;
    for t in tags(html) {
        out.push_str(&unescape(&html[pos..t.at]));
        out.push(' ');
        pos = t.end;
    }
    out.push_str(&unescape(&html[pos..]));
    out
}

fn starts<'a>(all: &'a [Tag], name: &str) -> Vec<&'a Tag> {
    all.iter().filter(|t| !t.closing && t.name == name).collect()
}

fn attr<'a>(tag: &'a Tag, name: &str) -> Option<&'a Option<String>> {
    tag.attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v)
}

fn inner_text(html: &str, all: &[Tag], open: &Tag) -> String {
    let close =
        all.iter().find(|t| t.closing && t.name == open.name && t.at >= open.end).unwrap_or_else(|| panic!("<{}> is never closed", open.name));
    text_of(&html[open.end..close.at])
}

fn is_global(attribute: &str) -> bool {
    matches!(attribute, "id" | "class" | "slot" | "title" | "hidden") || attribute.starts_with("aria-") || attribute.starts_with("data-")
}

fn plan_tags(plan: &ExportPlan) -> BTreeSet<String> {
    plan.components.iter().filter_map(|c| c.tag.clone()).collect()
}

fn component_elements<'a>(all: &'a [Tag], plan: &ExportPlan) -> Vec<&'a Tag> {
    let known = plan_tags(plan);
    all.iter().filter(|t| !t.closing && known.contains(&t.name)).collect()
}

fn attribute_names(all: &[Tag], plan: &ExportPlan) -> Vec<String> {
    component_elements(all, plan).iter().flat_map(|t| t.attrs.iter().map(|(n, _)| n.clone())).collect()
}

fn markers(all: &[Tag]) -> Vec<(&Tag, String)> {
    all.iter().filter(|t| !t.closing).filter_map(|t| attr(t, "data-uigen-missing").map(|v| (t, v.clone().unwrap_or_default()))).collect()
}

fn assert_safe(label: &str, html: &str) {
    for t in &tags(html) {
        assert_ne!(t.name, "script", "{label}: a <script> in the output");
        for (name, _) in &t.attrs {
            assert!(!name.starts_with("on"), "{label}: the event attribute {name} on <{}>", t.name);
        }
    }
    let lower = html.to_ascii_lowercase();
    for remote in ["http://", "https://", "url(//", "@import", "javascript:", "src=\"//", "href=\"//"] {
        assert!(!lower.contains(remote), "{label}: {remote} in the output");
    }
}

fn sections<'a>(html: &'a str, all: &[Tag]) -> Vec<&'a str> {
    let opens: Vec<&Tag> = starts(all, "section");
    let closes: Vec<&Tag> = all.iter().filter(|t| t.closing && t.name == "section").collect();
    assert_eq!(opens.len(), closes.len(), "unbalanced <section> in {html}");
    opens.iter().zip(closes).map(|(o, c)| &html[o.end..c.at]).collect()
}

fn title_of(section: &str, tag: &str) -> String {
    let all = tags(section);
    let first = starts(&all, tag).into_iter().next().unwrap_or_else(|| panic!("no <{tag}> in the section {section}"));
    text_of(&section[..first.at])
}

fn dashboard_plan() -> ExportPlan {
    plan_named(&["Button", "Card", "Text field"])
}

// covers: E7-3.1
#[test]
fn a_requested_component_becomes_the_tag_of_the_plan_with_its_children_kept() {
    let plan = dashboard_plan();
    let composed = compose(Template::Dashboard, &plan);
    assert!(!composed.html.contains("uigen-use"), "a <uigen-use> is left: {}", composed.html);
    let all = tags(&composed.html);
    let buttons = starts(&all, "ui-button");
    assert!(!buttons.is_empty(), "no <ui-button> in {}", composed.html);
    let closes = all.iter().filter(|t| t.closing && t.name == "ui-button").count();
    assert_eq!(closes, buttons.len(), "each <ui-button> is closed by </ui-button>");
    assert!(buttons.iter().any(|b| !inner_text(&composed.html, &all, b).trim().is_empty()), "the children of a button are gone");
    let known = plan_tags(&plan);
    for t in all.iter().filter(|t| t.name.contains('-') && t.name != "!--") {
        assert!(known.contains(&t.name), "<{}> is not a tag of the plan", t.name);
    }
}

// covers: E7-3.1
#[test]
fn every_name_a_template_asks_for_resolves_to_a_component_of_that_name() {
    for page in &TEMPLATE_NAMES[1..] {
        let names = requested(page);
        assert!(!names.is_empty(), "the {page} template asks for no component");
        for name in names {
            let plan = plan_named(&[&name]);
            let composed = compose(template(page), &plan);
            assert!(!composed.missing.contains(&name), "{page}: {name} is still missing with a component of that name");
            let tag = format!("ui-{}", slug(&name));
            assert!(!starts(&tags(&composed.html), &tag).is_empty(), "{page}: no <{tag}> in {}", composed.html);
        }
    }
}

// covers: E7-3.1
#[test]
fn names_compare_without_case_spaces_or_hyphens() {
    let plain = plan_named(&["Button", "Card", "Text field"]);
    let spelled = plan_of(vec![
        planned(1, "BUTTON", "ui-button", Status::Ok, vec![recipe(1, &[], &[])]),
        planned(2, "c a r d", "ui-card", Status::Ok, vec![recipe(2, &[], &[])]),
        planned(3, "TEXT-FIELD", "ui-text-field", Status::Ok, vec![recipe(3, &[], &[])]),
    ]);
    assert!(!starts(&tags(&compose(Template::Dashboard, &plain).html), "ui-button").is_empty(), "the reference plan gives no button");
    for page in &TEMPLATE_NAMES[1..] {
        let (a, b) = (compose(template(page), &plain), compose(template(page), &spelled));
        assert_eq!(a.html, b.html, "{page}: the spelling of a name changes the page");
        assert_eq!(a.missing, b.missing, "{page}: the spelling of a name changes what is missing");
    }
}

// covers: E7-3.1
#[test]
fn a_different_name_is_not_a_match() {
    let name = requested("dashboard").into_iter().find(|n| norm(n) == "button").expect("the dashboard asks for a Button");
    let composed = compose(Template::Dashboard, &plan_named(&["Buttons"]));
    assert!(composed.missing.contains(&name), "Buttons must not stand for {name}: missing is {:?}", composed.missing);
    assert!(starts(&tags(&composed.html), "ui-buttons").is_empty());
}

// covers: E7-3.2
#[test]
fn a_component_the_plan_lacks_gets_a_marker_with_the_fixed_text() {
    let plan = dashboard_plan();
    let mut asking_for_a_chart = 0;
    for page in &TEMPLATE_NAMES[1..] {
        let composed = compose(template(page), &plan);
        let all = tags(&composed.html);
        let found = markers(&all);
        for name in &composed.missing {
            let (marker, _) = found.iter().find(|(_, v)| v == name).unwrap_or_else(|| panic!("{page}: no marker for {name} in {}", composed.html));
            assert_eq!(marker.name, "div", "{page}: the marker of {name}");
            assert_eq!(attr(marker, "role"), Some(&Some("note".to_string())), "{page}: the marker of {name} is a note");
            assert_eq!(inner_text(&composed.html, &all, marker).trim(), format!("{MARKER_TEXT}{name}"), "{page}: the text of the marker of {name}");
        }
        for (_, name) in &found {
            assert!(composed.missing.contains(name), "{page}: a marker for {name}, which missing does not list");
        }
        if composed.missing.iter().any(|n| norm(n) == "chart") {
            asking_for_a_chart += 1;
            assert!(composed.missing.contains(&"Chart".to_string()), "{page}: the chart is listed under another name: {:?}", composed.missing);
        }
    }
    assert!(asking_for_a_chart > 0, "no template asks for a Chart");
}

// covers: E7-3.2
#[test]
fn missing_lists_each_name_once_in_order_of_appearance() {
    for page in &TEMPLATE_NAMES[1..] {
        for plan in [plan_of(vec![]), dashboard_plan()] {
            let composed = compose(template(page), &plan);
            assert!(!composed.html.trim().is_empty(), "{page}: an empty page");
            let unique: BTreeSet<String> = composed.missing.iter().map(|n| norm(n)).collect();
            assert_eq!(unique.len(), composed.missing.len(), "{page}: duplicates in {:?}", composed.missing);
            let all = tags(&composed.html);
            let found = markers(&all);
            let first: Vec<usize> = composed
                .missing
                .iter()
                .map(|n| found.iter().position(|(_, v)| v == n).unwrap_or_else(|| panic!("{page}: no marker for {n}")))
                .collect();
            assert!(first.windows(2).all(|w| w[0] < w[1]), "{page}: missing {:?} is not in order of appearance {first:?}", composed.missing);
        }
    }
}

// covers: E7-3.2
#[test]
fn nothing_native_or_foreign_takes_the_place_of_a_missing_component() {
    for page in &TEMPLATE_NAMES[1..] {
        for plan in [plan_of(vec![]), dashboard_plan()] {
            let composed = compose(template(page), &plan);
            assert!(!composed.html.trim().is_empty(), "{page}: an empty page");
            let all = tags(&composed.html);
            let known = plan_tags(&plan);
            for t in all.iter().filter(|t| !t.closing) {
                assert!(!NATIVE_ELEMENTS.contains(&t.name.as_str()), "{page}: a native <{}> in {}", t.name, composed.html);
                assert!(!t.name.contains('-') || t.name == "!--" || known.contains(&t.name), "{page}: <{}> is not a tag of the plan", t.name);
            }
        }
    }
}

// covers: E7-3.3
#[test]
fn an_attribute_the_component_does_not_know_is_dropped_and_noted_never_invented() {
    let names = requested_by_the_pages();
    let (bare, rich) = (plan_with_keys(&names, &[]), plan_with_keys(&names, &PROPERTY_KEYS));
    let mut kept_somewhere = false;
    for page in &TEMPLATE_NAMES[1..] {
        let (without, with) = (compose(template(page), &bare), compose(template(page), &rich));
        let (a, b) = (attribute_names(&tags(&without.html), &bare), attribute_names(&tags(&with.html), &rich));
        for name in &a {
            assert!(is_global(name), "{page}: the attribute {name} reaches a component that has no such property");
        }
        for name in &b {
            assert!(is_global(name) || PROPERTY_KEYS.contains(&name.as_str()), "{page}: the attribute {name} is invented");
        }
        assert!(b.len() >= a.len(), "{page}: knowing more properties drops attributes: {a:?} then {b:?}");
        for name in b.iter().filter(|n| !is_global(n)) {
            kept_somewhere = true;
            assert!(
                without.notes.iter().any(|note| note.contains("unknown-prop") && note.contains(name.as_str())),
                "{page}: the dropped attribute {name} is not noted as unknown-prop in {:?}",
                without.notes
            );
        }
    }
    assert!(kept_somewhere, "no template passes a variant attribute to a component");
}

// covers: E7-3.3
#[test]
fn a_variant_property_of_the_component_passes_while_another_components_does_not() {
    let names = requested_by_the_pages();
    let plan = plan_of(
        names
            .iter()
            .zip(1u64..)
            .map(|(name, id)| {
                let keys: &[(&str, &str)] = if norm(name) == "button" { &[("variant", "v")] } else { &[] };
                ok(id, name, vec![recipe(id, keys, keys)])
            })
            .collect(),
    );
    let mut passed = false;
    for page in &TEMPLATE_NAMES[1..] {
        let composed = compose(template(page), &plan);
        let all = tags(&composed.html);
        for t in component_elements(&all, &plan) {
            for (name, _) in t.attrs.iter().filter(|(n, _)| !is_global(n)) {
                assert!(t.name == "ui-button" && name == "variant", "{page}: <{}> got the attribute {name}", t.name);
                passed = true;
            }
        }
    }
    assert!(passed, "no template passes a variant to a Button");
}

fn components_plan() -> ExportPlan {
    plan_of(vec![
        ok(
            1,
            "Button",
            vec![
                recipe(11, &[("variant", "primary")], &[("variant", "primary")]),
                recipe(12, &[("variant", "secondary")], &[("variant", "secondary")]),
                recipe(13, &[("variant", "primary")], &[("variant", "primary"), ("disabled", "")]),
            ],
        ),
        ok(2, "Card", vec![recipe(21, &[], &[])]),
        planned(3, "Empty thing", "ui-empty-thing", Status::Empty, vec![recipe(31, &[], &[])]),
        planned(4, "Broken", "ui-broken", Status::Invalid, vec![]),
    ])
}

// covers: E7-3.4
#[test]
fn the_components_page_has_a_titled_section_per_exportable_component_with_an_instance_per_variant() {
    let plan = components_plan();
    let composed = compose(Template::Components, &plan);
    let all = tags(&composed.html);
    let parts = sections(&composed.html, &all);
    assert_eq!(parts.len(), 2, "one section per exportable component: {}", composed.html);
    assert!(title_of(parts[0], "ui-button").contains("Button"), "the title of the first section: {}", parts[0]);
    assert!(title_of(parts[1], "ui-card").contains("Card"), "the title of the second section: {}", parts[1]);
    let buttons = starts(&all, "ui-button");
    assert_eq!(buttons.len(), 3, "one instance per variant of the Button");
    assert_eq!(starts(&all, "ui-card").len(), 1);
    let variants: Vec<Option<&Option<String>>> = buttons.iter().map(|b| attr(b, "variant")).collect();
    assert_eq!(variants, [Some(&Some("primary".to_string())), Some(&Some("secondary".to_string())), Some(&Some("primary".to_string()))]);
    assert!(attr(buttons[0], "disabled").is_none() && attr(buttons[1], "disabled").is_none(), "only the third variant is disabled");
    let boolean = attr(buttons[2], "disabled").unwrap_or_else(|| panic!("the disabled variant has no disabled attribute"));
    assert!(boolean.as_deref().is_none_or(str::is_empty), "a boolean attribute has an empty value, got {boolean:?}");
    assert!(composed.missing.is_empty(), "the Components page asks for nothing: {:?}", composed.missing);
}

// covers: E7-3.4
#[test]
fn an_empty_or_invalid_component_has_no_section() {
    let composed = compose(Template::Components, &components_plan());
    let all = tags(&composed.html);
    assert!(!starts(&all, "ui-button").is_empty(), "the exportable components are shown: {}", composed.html);
    assert!(starts(&all, "ui-empty-thing").is_empty() && starts(&all, "ui-broken").is_empty(), "{}", composed.html);
    let text = text_of(&composed.html);
    assert!(!text.contains("Empty thing") && !text.contains("Broken"), "an excluded component is named in {text}");
}

// covers: E7-3.4
#[test]
fn a_plan_without_an_exportable_component_gives_an_empty_page_and_a_note() {
    let nothing = plan_of(vec![]);
    let excluded =
        plan_of(vec![planned(3, "Empty thing", "ui-empty-thing", Status::Empty, vec![]), planned(4, "Broken", "ui-broken", Status::Invalid, vec![])]);
    for plan in [nothing, excluded] {
        let composed = compose(Template::Components, &plan);
        assert!(composed.html.trim().is_empty(), "expected an empty page, got {}", composed.html);
        assert!(!composed.notes.is_empty(), "an empty page comes with a note");
        assert!(composed.missing.is_empty());
    }
}

// covers: E7-3.5
#[test]
fn every_template_composes_against_a_full_plan_without_a_marker() {
    let plan = full_plan();
    for page in TEMPLATE_NAMES {
        let composed = compose(template(page), &plan);
        assert!(composed.missing.is_empty(), "{page}: missing {:?} with a full plan", composed.missing);
        assert!(!composed.html.contains("data-uigen-missing"), "{page}: a marker with a full plan");
        assert!(!composed.html.contains("uigen-use"), "{page}: a <uigen-use> is left");
        assert!(!composed.html.trim().is_empty(), "{page}: an empty page");
        assert_safe(page, &composed.html);
    }
    let components = compose(Template::Components, &plan);
    assert_eq!(sections(&components.html, &tags(&components.html)).len(), plan.components.len());
}

// covers: E7-3.5
#[test]
fn against_a_plan_reduced_to_a_button_each_template_lists_exactly_the_absent_names() {
    let plan = plan_named(&["Button"]);
    for page in TEMPLATE_NAMES {
        let composed = compose(template(page), &plan);
        let expected: Vec<String> = requested(page).into_iter().filter(|n| norm(n) != "button").collect();
        assert_eq!(composed.missing, expected, "{page}: missing");
        let found: BTreeSet<String> = markers(&tags(&composed.html)).into_iter().map(|(_, v)| v).collect();
        assert_eq!(found, expected.iter().cloned().collect::<BTreeSet<_>>(), "{page}: the markers");
        assert!(!composed.html.trim().is_empty(), "{page}: an empty page");
        assert_safe(page, &composed.html);
    }
    let components = compose(Template::Components, &plan);
    assert!(!starts(&tags(&components.html), "ui-button").is_empty(), "the Components page shows the button");
}

// covers: E7-3.5
#[test]
fn the_templates_stay_safe_whatever_the_plan() {
    for plan in [plan_of(vec![]), plan_named(&["Button"]), full_plan(), plan_with_keys(&requested_by_the_pages(), &PROPERTY_KEYS)] {
        for page in TEMPLATE_NAMES {
            let composed = compose(template(page), &plan);
            assert!((page == "components" && plan.components.len() != 1) || !composed.html.trim().is_empty(), "{page}: an empty page");
            assert_safe(page, &composed.html);
        }
    }
}

// covers: E7-3.6
#[test]
fn names_and_values_with_quotes_brackets_and_ampersands_are_escaped() {
    let evil_name = "Evil \"quote\" <b>bold</b> & co";
    let evil_value = "a\"b<c&d";
    let trapped = plan_of(vec![ok(
        1,
        evil_name,
        vec![
            recipe(11, &[("variant", evil_value)], &[("variant", evil_value)]),
            recipe(12, &[("variant", "x")], &[("variant", "x"), ("disabled", "")]),
        ],
    )]);
    let benign = plan_of(vec![planned(
        1,
        "Evil",
        &format!("ui-{}", slug(evil_name)),
        Status::Ok,
        vec![recipe(11, &[("variant", "v")], &[("variant", "v")]), recipe(12, &[("variant", "x")], &[("variant", "x"), ("disabled", "")])],
    )]);
    let (composed, reference) = (compose(Template::Components, &trapped), compose(Template::Components, &benign));
    let (all, plain) = (tags(&composed.html), tags(&reference.html));
    let names = |tags: &[Tag]| tags.iter().filter(|t| !t.closing).map(|t| t.name.clone()).collect::<BTreeSet<_>>();
    assert_eq!(names(&all), names(&plain), "a name or a value added an element: {}", composed.html);
    assert!(!composed.html.contains("<b>"), "a raw <b> in {}", composed.html);
    let tag = format!("ui-{}", slug(evil_name));
    let instances = starts(&all, &tag);
    assert_eq!(instances.len(), 2, "one instance per variant in {}", composed.html);
    assert_eq!(attr(instances[0], "variant"), Some(&Some(evil_value.to_string())), "the value round-trips through the escaping");
    assert!(text_of(&composed.html).contains(evil_name), "the name reads back as written in {}", composed.html);
    assert_safe("trapped", &composed.html);
}
