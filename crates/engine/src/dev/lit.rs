//! Lit web components: a component (or component set) becomes a `LitElement` in TypeScript. Its
//! variant, boolean and text properties are reflected `@property` fields (a name that would shadow
//! a member of `HTMLElement` or `LitElement` gets a `Variant` or `Value` suffix), its slots and
//! instance swaps are named `<slot>` elements holding their default content, the differences
//! between variants are `:host([attribute="value"])` rules, and an instance is the custom element
//! with the values that differ from the defaults. Built on the same walk as the React, Vue and
//! Leptos output ([`crate::component_codegen`], [`crate::codegen::Markup`]).

use figcraft_doc::{Document, NodeId, NodeKind};
use serde::Serialize;
use serde_json::Value;

use crate::codegen::{CodeLang, Ctx, Markup, class_name};
use crate::component_codegen::{Component, Param, ParamKind, js_string, web};
use crate::components::{slots_of, variant_set};
use crate::params::id_of;

/// Longest custom element name (chars).
const MAX_TAG: usize = 64;

/// Names of custom elements the HTML specification reserves.
const RESERVED_TAGS: &[&str] =
    &["annotation-xml", "color-profile", "font-face", "font-face-src", "font-face-uri", "font-face-format", "font-face-name", "missing-glyph"];

/// Members of `HTMLElement` and `LitElement` a generated property may not take.
const HOST_MEMBERS: &[&str] = &[
    "accessKey",
    "addController",
    "after",
    "animate",
    "append",
    "attributes",
    "autofocus",
    "before",
    "blur",
    "children",
    "classList",
    "className",
    "click",
    "closest",
    "connectedCallback",
    "contains",
    "contentEditable",
    "dataset",
    "dir",
    "disconnectedCallback",
    "draggable",
    "firstUpdated",
    "focus",
    "hasUpdated",
    "hidden",
    "id",
    "inert",
    "innerHTML",
    "innerText",
    "isConnected",
    "lang",
    "matches",
    "nonce",
    "outerHTML",
    "part",
    "popover",
    "prepend",
    "remove",
    "render",
    "renderRoot",
    "requestUpdate",
    "role",
    "shadowRoot",
    "slot",
    "spellcheck",
    "style",
    "tabIndex",
    "title",
    "translate",
    "update",
    "updateComplete",
    "updated",
    "willUpdate",
];

/// Where Lit is generated: custom element names start with `prefix` (`ui-`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LitOptions {
    pub prefix: String,
    /// The prefix of the custom properties design tokens become (`ui` → `--ui-colors-brand`).
    pub token_prefix: String,
}

impl Default for LitOptions {
    fn default() -> Self {
        LitOptions::new("ui-")
    }
}

impl LitOptions {
    /// Options for `prefix`; tokens take it without its hyphens.
    pub fn new(prefix: &str) -> Self {
        LitOptions { prefix: prefix.to_string(), token_prefix: prefix.trim_matches('-').to_string() }
    }
}

/// Whether `prefix` can start a custom element name: lowercase letters, digits and hyphens.
pub fn valid_prefix(prefix: &str) -> bool {
    prefix.chars().count() <= 20 && prefix.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Why a component name can't be a custom element name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagIssue {
    Empty,
    LeadingDigit,
    Reserved,
    NoHyphen,
    TooLong,
}

/// Something the generated code can't reproduce, or a choice made on the user's behalf.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Note {
    pub code: &'static str,
    pub component: String,
    pub layer: Option<String>,
    pub message: String,
}

/// A component as a Lit file.
#[derive(Clone, Debug, PartialEq)]
pub struct LitOutput {
    pub tag: String,
    pub class_name: String,
    pub ts: String,
    pub notes: Vec<Note>,
}

/// The custom element name of a component called `name`: `prefix` and the name as a kebab-case slug.
pub fn tag_name(name: &str, prefix: &str) -> Result<String, TagIssue> {
    let mut slug = String::new();
    for c in deunicode::deunicode(name).chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        return Err(TagIssue::Empty);
    }
    let prefix: String = prefix.chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-').collect();
    let tag = format!("{prefix}{slug}");
    if !tag.starts_with(|c: char| c.is_ascii_lowercase()) {
        return Err(TagIssue::LeadingDigit);
    }
    if RESERVED_TAGS.contains(&tag.as_str()) {
        return Err(TagIssue::Reserved);
    }
    if !tag.contains('-') {
        return Err(TagIssue::NoHyphen);
    }
    if tag.chars().count() > MAX_TAG {
        return Err(TagIssue::TooLong);
    }
    Ok(tag)
}

/// The class a custom element is declared with: `ui-button` → `UiButton`.
fn class_of(tag: &str) -> String {
    tag.split('-')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
        })
        .collect()
}

/// The name a component, or a set's variant, is known by.
fn name_of(doc: &Document, comp: NodeId) -> Option<String> {
    let n = doc.node(comp).filter(|n| matches!(n.kind, NodeKind::Component { .. } | NodeKind::ComponentSet { .. }))?;
    Some(variant_set(doc, comp).and_then(|s| doc.node(s)).unwrap_or(n).name.clone())
}

/// The custom element of the component `comp` (`None` when its name makes no valid tag).
pub(crate) fn tag_of(doc: &Document, comp: NodeId, opts: &LitOptions) -> Option<String> {
    tag_name(&name_of(doc, comp)?, &opts.prefix).ok()
}

/// `c`'s custom element name; a component whose name makes no valid tag is `<prefix>component`.
pub(crate) fn tag_for(doc: &Document, c: &Component, opts: &LitOptions) -> String {
    tag_of(doc, c.set.unwrap_or(c.comp), opts).or_else(|| tag_name("component", &opts.prefix).ok()).unwrap_or_else(|| "ui-component".to_string())
}

// ------------------------------------------------------------------ properties

/// What a parameter is in the class: its member name, its attribute (property kinds only) and
/// whether it is reflected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub ident: String,
    pub attribute: Option<String>,
    pub reflect: bool,
}

fn is_property(p: &Param) -> bool {
    matches!(p.kind, ParamKind::Boolean | ParamKind::Text | ParamKind::Variant(_))
}

/// A parameter's member name: its own, or with `Variant` (variant properties) or `Value` (the
/// others) when it would shadow a member of `HTMLElement` or `LitElement`.
pub fn prop_name(p: &Param) -> String {
    if !HOST_MEMBERS.contains(&p.ident.as_str()) {
        return p.ident.clone();
    }
    let suffix = if matches!(p.kind, ParamKind::Variant(_)) { "Variant" } else { "Value" };
    format!("{}{suffix}", p.ident)
}

/// `name`, or `name2`, `name3`… when taken; remembered in `used`.
fn unique(name: &str, sep: &str, used: &mut Vec<String>) -> String {
    let out = (1..).map(|k| if k == 1 { name.to_string() } else { format!("{name}{sep}{k}") }).find(|n| !used.contains(n)).unwrap_or_default();
    used.push(out.clone());
    out
}

/// An attribute name from a member name: `showIcon` → `show-icon`, `showURL` → `show-url`.
fn kebab(ident: &str) -> String {
    let chars: Vec<char> = ident.chars().collect();
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev = i.checked_sub(1).and_then(|j| chars.get(j));
            let boundary = prev.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
                || (prev.is_some_and(char::is_ascii_uppercase) && chars.get(i + 1).is_some_and(char::is_ascii_lowercase));
            if boundary {
                out.push('-');
            }
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// One [`Member`] per parameter of `c`, in order, with unique member and attribute names.
pub fn members(c: &Component) -> Vec<Member> {
    let mut used: Vec<String> = c.params.iter().filter(|p| prop_name(p) == p.ident).map(|p| p.ident.clone()).collect();
    let mut attributes: Vec<String> = Vec::new();
    c.params
        .iter()
        .map(|p| {
            let ident = if prop_name(p) == p.ident { p.ident.clone() } else { unique(&prop_name(p), "", &mut used) };
            let attribute = is_property(p).then(|| unique(&kebab(&ident), "-", &mut attributes));
            Member { ident, attribute, reflect: matches!(p.kind, ParamKind::Variant(_)) }
        })
        .collect()
}

/// The member of parameter `p` in `c`.
pub(crate) fn member(c: &Component, p: &Param) -> Member {
    let all = members(c);
    c.params.iter().position(|q| q.prop == p.prop).and_then(|i| all.into_iter().nth(i)).unwrap_or(Member {
        ident: p.ident.clone(),
        attribute: None,
        reflect: false,
    })
}

/// The `<slot name="…">` of a slot or instance swap parameter, unique among the component's.
pub(crate) fn slot_name(c: &Component, p: &Param) -> String {
    let mut used = Vec::new();
    for q in c.params.iter().filter(|q| matches!(q.kind, ParamKind::Slot | ParamKind::InstanceSwap)) {
        let name = class_name(&q.ident, "slot", &mut used);
        if q.prop == p.prop {
            return name;
        }
    }
    class_name(&p.ident, "slot", &mut Vec::new())
}

// ------------------------------------------------------------------ text

/// A TypeScript string literal in which a backtick or `${` can never open a template.
pub(crate) fn ts_string(s: &str) -> String {
    js_string(s).replace('`', "\\`").replace("${", "\\${")
}

/// `s` as the text of a TypeScript template literal.
pub(crate) fn template_text(s: &str) -> String {
    s.replace('\\', "\\\\").replace('`', "\\`").replace("${", "\\${")
}

/// Layer text as the content of an element in a `html` template.
pub(crate) fn text(s: &str) -> String {
    template_text(&crate::codegen::html_escape(s))
}

fn attribute_value(s: &str, template: bool) -> String {
    let escaped = s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;");
    if template { template_text(&escaped) } else { escaped }
}

/// A CSS string's content: `\`, `"` and line breaks escaped.
pub(crate) fn css_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\a ").replace('\r', "")
}

// ------------------------------------------------------------------ definition

fn field(p: &Param, m: &Member) -> Option<String> {
    let explicit = m.attribute.as_deref().filter(|a| *a != m.ident);
    let default_true = p.kind == ParamKind::Boolean && p.default.as_bool() == Some(true);
    let mut options: Vec<String> = Vec::new();
    if p.kind == ParamKind::Boolean {
        options.push("type: Boolean".into());
    }
    if let Some(a) = explicit {
        options.push(format!("attribute: \"{a}\""));
    }
    if default_true {
        options.push("converter: { fromAttribute: (value: string | null) => value !== \"false\" }".into());
    }
    if m.reflect {
        options.push("reflect: true".into());
    }
    let decorator = if options.is_empty() { "@property()".to_string() } else { format!("@property({{ {} }})", options.join(", ")) };
    let name = &m.ident;
    match &p.kind {
        ParamKind::Boolean => Some(format!("{decorator} {name} = {};", p.default.as_bool().unwrap_or(false))),
        ParamKind::Text => Some(format!("{decorator} {name} = {};", ts_string(p.default.as_str().unwrap_or("")))),
        ParamKind::Variant(opts) => {
            let union = if opts.is_empty() { "string".to_string() } else { opts.iter().map(|o| ts_string(o)).collect::<Vec<_>>().join(" | ") };
            Some(format!("{decorator} {name}: {union} = {};", ts_string(p.default.as_str().unwrap_or(""))))
        }
        ParamKind::InstanceSwap | ParamKind::Slot => None,
    }
}

/// `c` as a Lit component.
pub fn definition(doc: &Document, c: &Component, opts: &LitOptions) -> LitOutput {
    let tag = tag_for(doc, c, opts);
    let class = class_of(&tag);
    let (_, markup, css) = web(doc, c, CodeLang::Lit, opts);
    let hugs = doc.node(c.comp).is_some_and(|n| n.layout.is_some() && n.child_layout.sizing_h == figcraft_doc::Sizing::Hug);
    let display = if hugs { "inline-block" } else { "block" };
    let mut out =
        String::from("import { LitElement, css, html, nothing } from \"lit\";\nimport { customElement, property } from \"lit/decorators.js\";\n\n");
    out.push_str(&format!("@customElement(\"{tag}\")\nexport class {class} extends LitElement {{\n"));
    let fields: Vec<String> = c.params.iter().zip(members(c)).filter_map(|(p, m)| field(p, &m)).collect();
    for f in &fields {
        out.push_str(&format!("  {f}\n"));
    }
    if !fields.is_empty() {
        out.push('\n');
    }
    out.push_str(&format!("  static styles = css`\n    :host {{ display: {display}; }}\n"));
    for line in template_text(&css).lines() {
        out.push_str(&if line.is_empty() { "\n".to_string() } else { format!("    {line}\n") });
    }
    out.push_str(&format!("  `;\n\n  render() {{\n    return html`\n{markup}    `;\n  }}\n}}\n\n"));
    out.push_str(&format!("declare global {{\n  interface HTMLElementTagNameMap {{\n    \"{tag}\": {class};\n  }}\n}}\n"));
    LitOutput { tag, class_name: class, ts: out, notes: Vec::new() }
}

// ------------------------------------------------------------------ instances

/// An instance swap's slot: its default component, as its custom element.
pub(crate) fn swap_slot(ctx: &Ctx, p: &Param, depth: usize, opts: &LitOptions) -> String {
    let name = ctx.comp.map_or_else(|| class_name(&p.ident, "slot", &mut Vec::new()), |c| slot_name(c, p));
    let default = id_of(&p.default).and_then(|id| tag_of(ctx.doc, id, opts)).map(|t| format!("<{t}></{t}>")).unwrap_or_default();
    format!("{}<slot name=\"{name}\">{default}</slot>\n", "  ".repeat(depth))
}

/// What an instance puts in a slot, in an element that leaves the layout to its children, and the
/// CSS of its layers.
fn slot_content(doc: &Document, layer: NodeId, name: &str, depth: usize, opts: &LitOptions, template: bool) -> (String, String) {
    let Some(n) = doc.node(layer) else { return (String::new(), String::new()) };
    let ctx = Ctx { doc, root: layer, comp: None, base: depth };
    let mut m = Markup::new(&ctx, CodeLang::Lit);
    m.class_prefix = Some(format!("slot-{name}-"));
    m.lit = opts.clone();
    m.lit_plain = !template;
    for k in ctx.kids(n) {
        m.node(k, Some(n), depth + 1, "");
    }
    let pad = "  ".repeat(depth);
    (format!("{pad}<div slot=\"{name}\" style=\"display: contents\">\n{}{pad}</div>\n", m.markup), m.styles)
}

/// The custom element for instance `id` of `c` at `depth`, with the values that differ from the
/// defaults as attributes and the content of its slots and swaps as children; and the CSS that
/// content needs. `template`: the call sits in a `html` template literal.
pub(crate) fn usage(
    doc: &Document,
    id: NodeId,
    c: &Component,
    values: &[(String, Value)],
    depth: usize,
    opts: &LitOptions,
    template: bool,
) -> (String, String) {
    let pad = "  ".repeat(depth);
    let tag = tag_for(doc, c, opts);
    let slots = slots_of(doc, id);
    let (mut attrs, mut children, mut css) = (String::new(), String::new(), String::new());
    for ((p, m), (_, v)) in c.params.iter().zip(members(c)).zip(values) {
        match &p.kind {
            ParamKind::Slot => {
                if v.as_bool() == Some(true)
                    && let Some((layer, _)) = slots.iter().find(|(_, name)| *name == p.prop)
                {
                    let (content, style) = slot_content(doc, *layer, &slot_name(c, p), depth + 1, opts, template);
                    children.push_str(&content);
                    css.push_str(&style);
                }
            }
            ParamKind::InstanceSwap => {
                if id_of(v) != id_of(&p.default)
                    && let Some(swapped) = id_of(v).and_then(|i| tag_of(doc, i, opts))
                {
                    children.push_str(&format!("{pad}  <{swapped} slot=\"{}\"></{swapped}>\n", slot_name(c, p)));
                }
            }
            _ if *v == p.default => {}
            ParamKind::Boolean => {
                let Some(attr) = m.attribute else { continue };
                if v.as_bool() == Some(true) {
                    attrs.push_str(&format!(" {attr}"));
                } else {
                    attrs.push_str(&format!(" {attr}=\"false\""));
                }
            }
            ParamKind::Text | ParamKind::Variant(_) => {
                let Some(attr) = m.attribute else { continue };
                attrs.push_str(&format!(" {attr}=\"{}\"", attribute_value(v.as_str().unwrap_or(""), template)));
            }
        }
    }
    let code = if children.is_empty() { format!("{pad}<{tag}{attrs}></{tag}>\n") } else { format!("{pad}<{tag}{attrs}>\n{children}{pad}</{tag}>\n") };
    (code, css)
}
