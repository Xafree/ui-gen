//! Lit web components: a component (or component set) becomes a `LitElement` in TypeScript. Its
//! variant, boolean and text properties are reflected `@property` fields (a name that would shadow
//! a member of `HTMLElement` or `LitElement` gets a `Variant` or `Value` suffix), its slots and
//! instance swaps are named `<slot>` elements holding their default content (a root that can hold content also gets a default `<slot>`), the differences
//! between variants are `:host([attribute="value"])` rules, and an instance is the custom element
//! with the values that differ from the defaults. Interaction states (hover, focus, pressed,
//! disabled, loading) are CSS pseudo-classes and reflected booleans rather than a `state` property.
//! Built on the same walk as the React, Vue and
//! Leptos output ([`crate::component_codegen`], [`crate::codegen::Markup`]).

use std::collections::{BTreeMap, HashMap, HashSet};

use figcraft_doc::{Document, NodeId, NodeKind};
use serde::Serialize;
use serde_json::{Value, json};

use crate::codegen::{CodeLang, Ctx, Markup, Tokens, class_name};
use crate::component_codegen::{Component, Param, ParamKind, describe, instance_values, js_string, main_of, web_tokens};
use crate::components::{slots_of, variant_props, variant_set};
use crate::params::id_of;

mod motion;

pub use motion::easing_css;
pub(crate) use motion::{Motion, plan as plan_motion};

/// Most variants of a set that get rules and recipes.
const MAX_VARIANTS: usize = 256;

/// Longest custom element name (chars).
const MAX_TAG: usize = 64;

/// Names of custom elements the HTML specification reserves.
const RESERVED_TAGS: &[&str] =
    &["annotation-xml", "color-profile", "font-face", "font-face-src", "font-face-uri", "font-face-format", "font-face-name", "missing-glyph"];

/// Members of `HTMLElement` and `LitElement` a generated property may not take.
const HOST_MEMBERS: &[&str] = &[
    "accessKey",
    "accessKeyLabel",
    "addController",
    "addEventListener",
    "adoptedCallback",
    "after",
    "animate",
    "append",
    "appendChild",
    "ariaAtomic",
    "ariaAutoComplete",
    "ariaBusy",
    "ariaChecked",
    "ariaColCount",
    "ariaColIndex",
    "ariaColSpan",
    "ariaCurrent",
    "ariaDescription",
    "ariaDisabled",
    "ariaExpanded",
    "ariaHasPopup",
    "ariaHidden",
    "ariaInvalid",
    "ariaKeyShortcuts",
    "ariaLabel",
    "ariaLevel",
    "ariaLive",
    "ariaModal",
    "ariaMultiLine",
    "ariaMultiSelectable",
    "ariaOrientation",
    "ariaPlaceholder",
    "ariaPosInSet",
    "ariaPressed",
    "ariaReadOnly",
    "ariaRelevant",
    "ariaRequired",
    "ariaRoleDescription",
    "ariaRowCount",
    "ariaRowIndex",
    "ariaRowSpan",
    "ariaSelected",
    "ariaSetSize",
    "ariaSort",
    "ariaValueMax",
    "ariaValueMin",
    "ariaValueNow",
    "ariaValueText",
    "attachShadow",
    "attributeChangedCallback",
    "attributes",
    "autocapitalize",
    "autocorrect",
    "autofocus",
    "baseURI",
    "before",
    "blur",
    "checkVisibility",
    "childElementCount",
    "childNodes",
    "children",
    "classList",
    "className",
    "click",
    "clientHeight",
    "clientLeft",
    "clientTop",
    "clientWidth",
    "cloneNode",
    "closest",
    "compareDocumentPosition",
    "computedStyleMap",
    "connectedCallback",
    "constructor",
    "contains",
    "contentEditable",
    "createRenderRoot",
    "dataset",
    "dir",
    "disconnectedCallback",
    "dispatchEvent",
    "draggable",
    "enableUpdating",
    "enterKeyHint",
    "firstChild",
    "firstElementChild",
    "firstUpdated",
    "focus",
    "getAnimations",
    "getAttribute",
    "getAttributeNames",
    "getBoundingClientRect",
    "getClientRects",
    "getRootNode",
    "getUpdateComplete",
    "hasAttribute",
    "hasAttributes",
    "hasChildNodes",
    "hasOwnProperty",
    "hasUpdated",
    "hidden",
    "hidePopover",
    "id",
    "inert",
    "innerHTML",
    "innerText",
    "inputMode",
    "insertAdjacentElement",
    "insertAdjacentHTML",
    "insertAdjacentText",
    "insertBefore",
    "isConnected",
    "isContentEditable",
    "isEqualNode",
    "isSameNode",
    "isUpdatePending",
    "lang",
    "lastChild",
    "lastElementChild",
    "localName",
    "matches",
    "namespaceURI",
    "nextElementSibling",
    "nextSibling",
    "nodeName",
    "nodeType",
    "nodeValue",
    "nonce",
    "normalize",
    "offsetHeight",
    "offsetLeft",
    "offsetParent",
    "offsetTop",
    "offsetWidth",
    "outerHTML",
    "outerText",
    "ownerDocument",
    "parentElement",
    "parentNode",
    "part",
    "performUpdate",
    "popover",
    "prefix",
    "prepend",
    "previousElementSibling",
    "previousSibling",
    "querySelector",
    "querySelectorAll",
    "remove",
    "removeAttribute",
    "removeChild",
    "removeController",
    "removeEventListener",
    "render",
    "renderOptions",
    "renderRoot",
    "replaceChild",
    "replaceChildren",
    "replaceWith",
    "requestFullscreen",
    "requestPointerLock",
    "requestUpdate",
    "role",
    "scheduleUpdate",
    "scroll",
    "scrollBy",
    "scrollHeight",
    "scrollIntoView",
    "scrollLeft",
    "scrollTo",
    "scrollTop",
    "scrollWidth",
    "setAttribute",
    "shadowRoot",
    "shouldUpdate",
    "showPopover",
    "slot",
    "spellcheck",
    "style",
    "syncFocus",
    "tabIndex",
    "tabindex",
    "tagName",
    "textContent",
    "title",
    "toLocaleString",
    "toString",
    "toggleAttribute",
    "togglePopover",
    "translate",
    "update",
    "updateComplete",
    "updated",
    "valueOf",
    "willUpdate",
    "writingSuggestions",
];

/// Class names a component may not take: what its file imports or relies on.
const RESERVED_CLASSES: &[&str] = &["Boolean", "HTMLElement", "HTMLElementTagNameMap", "LitElement", "Map", "PropertyKey", "String"];

/// Most nodes visited when looking for the components a component uses.
const MAX_VISITED: usize = 1_000_000;

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
    /// The custom properties the component reads, in first-use order.
    pub tokens: Vec<String>,
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
    if RESERVED_CLASSES.contains(&class_of(&tag).as_str()) {
        return Err(TagIssue::Reserved);
    }
    Ok(tag)
}

/// The class a custom element is declared with: `ui-button` → `UiButton`.
pub(crate) fn class_of(tag: &str) -> String {
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

/// The component or set `id` belongs to, when it is one.
pub(crate) fn entry_of(doc: &Document, id: NodeId) -> Option<NodeId> {
    doc.node(id).filter(|n| matches!(n.kind, NodeKind::Component { .. } | NodeKind::ComponentSet { .. }))?;
    Some(variant_set(doc, id).unwrap_or(id))
}

/// The components `c` draws as instances, the ones its instance swaps default to and the ones its
/// instances swap in.
pub(crate) fn dependencies(doc: &Document, c: &Component) -> Vec<NodeId> {
    let mut stack = c.set.and_then(|s| doc.node(s)).map_or_else(|| vec![c.comp], |s| s.children.clone());
    let mut out: Vec<NodeId> = c.params.iter().filter(|p| p.kind == ParamKind::InstanceSwap).filter_map(|p| id_of(&p.default)).collect();
    let mut seen = HashSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) || seen.len() > MAX_VISITED {
            continue;
        }
        let Some(n) = doc.node(id) else { continue };
        if let NodeKind::Instance { component, .. } = &n.kind {
            out.push(*component);
            if let Some(inner) = main_of(doc, id).and_then(|m| describe(doc, m))
                && let Some(values) = instance_values(doc, id, &inner)
            {
                let swaps = inner.params.iter().zip(&values).filter(|(p, _)| p.kind == ParamKind::InstanceSwap);
                out.extend(swaps.filter_map(|(_, (_, v))| id_of(v)));
            }
        }
        stack.extend(n.children.iter().rev().copied());
    }
    out.into_iter().filter_map(|id| entry_of(doc, id)).collect()
}

/// The `import "./<tag>.js";` lines of the components `c` uses that have a file of their own.
fn dependency_imports(doc: &Document, c: &Component, tag: &str, opts: &LitOptions) -> String {
    let has_content =
        |id: NodeId| main_of(doc, id).and_then(|m| describe(doc, m)).is_some_and(|d| doc.node(d.comp).is_some_and(|n| !n.children.is_empty()));
    let mut tags: Vec<String> = dependencies(doc, c).into_iter().filter(|id| has_content(*id)).filter_map(|id| tag_of(doc, id, opts)).collect();
    tags.sort();
    tags.dedup();
    tags.iter()
        .filter(|t| *t != tag)
        .map(|t| {
            format!(
                "import \"./{t}.js\";
"
            )
        })
        .collect()
}

// ------------------------------------------------------------------ properties

/// What a parameter is in the class: its member name, its attribute (property kinds only) and
/// whether it is reflected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub ident: String,
    pub attribute: Option<String>,
    pub reflect: bool,
    /// A state property whose values are all pseudo-classes or booleans: the class has no member.
    pub hidden: bool,
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
    layout(c).members
}

fn named_members(c: &Component, hidden: Option<usize>) -> Vec<Member> {
    let mut used: Vec<String> =
        c.params.iter().enumerate().filter(|(i, p)| Some(*i) != hidden && prop_name(p) == p.ident).map(|(_, p)| p.ident.clone()).collect();
    let mut attributes: Vec<String> = Vec::new();
    c.params
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if Some(i) == hidden {
                return Member { ident: p.ident.clone(), attribute: None, reflect: false, hidden: true };
            }
            let ident = if prop_name(p) == p.ident { p.ident.clone() } else { unique(&prop_name(p), "", &mut used) };
            let attribute = is_property(p).then(|| unique(&kebab(&ident), "-", &mut attributes));
            Member { ident, attribute, reflect: matches!(p.kind, ParamKind::Variant(_)), hidden: false }
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
        hidden: false,
    })
}

/// Whether a component's root frame can hold content: it lays its children out, or has a layer
/// that is more than a drawn shape. An icon or an empty box takes none.
pub(crate) fn takes_content(doc: &Document, root: &figcraft_doc::Node) -> bool {
    let is_shape = |k: &NodeKind| {
        matches!(
            k,
            NodeKind::Rectangle
                | NodeKind::Ellipse { .. }
                | NodeKind::Polygon { .. }
                | NodeKind::Star { .. }
                | NodeKind::Line
                | NodeKind::Vector { .. }
                | NodeKind::BooleanOp { .. }
        )
    };
    matches!(root.kind, NodeKind::Frame | NodeKind::Component { .. })
        && (root.layout.is_some() || root.children.iter().filter_map(|k| doc.node(*k)).any(|k| !is_shape(&k.kind)))
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

// ------------------------------------------------------------------ states

/// What a value of a state property is on the web.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateKind {
    Rest,
    Hover,
    FocusVisible,
    Active,
    Disabled,
    Loading,
    Other,
}

const STATE_PROPERTIES: &[&str] = &["state", "status", "etat"];

fn fold(s: &str) -> String {
    deunicode::deunicode(s).trim().to_lowercase()
}

/// What `value` of the variant property `prop` means as an interaction state (`None` when `prop`
/// is not called State, Status or État; English and French, accents and case ignored).
pub fn state_kind(prop: &str, value: &str) -> Option<StateKind> {
    if !STATE_PROPERTIES.contains(&fold(prop).as_str()) {
        return None;
    }
    Some(match fold(value).as_str() {
        "default" | "rest" | "normal" | "enabled" | "repos" | "defaut" => StateKind::Rest,
        "hover" | "hovered" | "survol" | "survole" => StateKind::Hover,
        "focus" | "focused" | "focus-visible" | "focus clavier" => StateKind::FocusVisible,
        "pressed" | "press" | "active" | "down" | "appuye" | "enfonce" => StateKind::Active,
        "disabled" | "desactive" | "inactive" => StateKind::Disabled,
        "loading" | "chargement" => StateKind::Loading,
        _ => StateKind::Other,
    })
}

/// A layer's rules: (path, class, declarations).
pub(crate) type Rule = (String, String, String);

/// The state property of a component: where it is, what its values mean.
struct StateSet {
    index: usize,
    prop: String,
    values: Vec<(String, StateKind)>,
    /// Every value is recognised: no `state` property is left.
    hidden: bool,
    /// The Disabled and Loading booleans: (kind, member name, attribute).
    booleans: Vec<(StateKind, String, String)>,
}

impl StateSet {
    fn kind(&self, value: &str) -> StateKind {
        self.values.iter().find(|(v, _)| v == value).map_or(StateKind::Other, |(_, k)| *k)
    }

    fn has(&self, kind: StateKind) -> bool {
        self.values.iter().any(|(_, k)| *k == kind)
    }

    fn boolean(&self, kind: StateKind) -> Option<&(StateKind, String, String)> {
        self.booleans.iter().find(|(k, ..)| *k == kind)
    }
}

/// A boolean the state set needs and the component doesn't declare.
struct Extra {
    kind: StateKind,
    param: Param,
    member: Member,
}

struct Layout {
    members: Vec<Member>,
    state: Option<StateSet>,
    extras: Vec<Extra>,
}

fn state_set(c: &Component) -> Option<StateSet> {
    c.set?;
    let (index, p, options) = c.params.iter().enumerate().find_map(|(i, p)| match &p.kind {
        ParamKind::Variant(options) if state_kind(&p.prop, "").is_some() => Some((i, p, options)),
        _ => None,
    })?;
    let values: Vec<(String, StateKind)> = options.iter().map(|o| (o.clone(), state_kind(&p.prop, o).unwrap_or(StateKind::Other))).collect();
    let hidden = !values.is_empty() && values.iter().all(|(_, k)| *k != StateKind::Other);
    Some(StateSet { index, prop: p.prop.clone(), values, hidden, booleans: Vec::new() })
}

fn layout(c: &Component) -> Layout {
    let Some(mut state) = state_set(c) else { return Layout { members: named_members(c, None), state: None, extras: Vec::new() } };
    let mut members = named_members(c, state.hidden.then_some(state.index));
    let mut extras = Vec::new();
    let mut used: Vec<String> = members.iter().filter(|m| !m.hidden).map(|m| m.ident.clone()).collect();
    let mut attributes: Vec<String> = members.iter().filter_map(|m| m.attribute.clone()).collect();
    for (kind, name) in [(StateKind::Disabled, "disabled"), (StateKind::Loading, "loading")] {
        if !state.has(kind) {
            continue;
        }
        let declared = c.params.iter().position(|p| p.kind == ParamKind::Boolean && fold(&p.prop) == name);
        if let Some(i) = declared
            && let Some(m) = members.get_mut(i)
        {
            m.reflect = true;
            state.booleans.push((kind, m.ident.clone(), m.attribute.clone().unwrap_or_else(|| name.to_string())));
            continue;
        }
        let ident = unique(name, "", &mut used);
        let attribute = unique(&kebab(&ident), "-", &mut attributes);
        state.booleans.push((kind, ident.clone(), attribute.clone()));
        let param = Param { prop: name.to_string(), ident: ident.clone(), kind: ParamKind::Boolean, default: json!(false), enum_name: String::new() };
        extras.push(Extra { kind, param, member: Member { ident, attribute: Some(attribute), reflect: true, hidden: false } });
    }
    Layout { members, state: Some(state), extras }
}

/// A property of the Lit class, for `inspect.componentCode`.
pub struct Exposed {
    pub name: String,
    pub param: String,
    pub kind: &'static str,
    pub default: Value,
    pub value: Option<Value>,
    pub member: Member,
}

/// The properties the Lit class has: the parameters but a state property made only of
/// pseudo-classes, then the booleans the states add. `values`: an instance's, in parameter order.
pub fn exposed(c: &Component, values: Option<&[(String, Value)]>) -> Vec<Exposed> {
    let lay = layout(c);
    let in_state = |kind: StateKind| {
        let active = lay
            .state
            .as_ref()
            .is_some_and(|s| values.is_some_and(|all| all.iter().any(|(name, v)| *name == s.prop && v.as_str().is_some_and(|v| s.kind(v) == kind))));
        json!(active)
    };
    let mut out: Vec<Exposed> = c
        .params
        .iter()
        .zip(&lay.members)
        .filter(|(_, m)| !m.hidden)
        .map(|(p, m)| Exposed {
            name: p.prop.clone(),
            param: p.ident.clone(),
            kind: p.kind.id(),
            default: p.default.clone(),
            value: values.map(|all| all.iter().find(|(name, _)| *name == p.prop).map_or(Value::Null, |(_, x)| x.clone())),
            member: m.clone(),
        })
        .collect();
    out.extend(lay.extras.iter().map(|e| Exposed {
        name: e.param.prop.clone(),
        param: e.param.ident.clone(),
        kind: e.param.kind.id(),
        default: e.param.default.clone(),
        value: values.map(|_| in_state(e.kind)),
        member: e.member.clone(),
    }));
    out
}

/// The attribute an instance showing the state `value` sets (Disabled, Loading), if any.
fn state_attribute(lay: &Layout, value: &Value) -> Option<String> {
    let state = lay.state.as_ref()?;
    let kind = state.kind(value.as_str()?);
    state.boolean(kind).map(|(_, _, attribute)| attribute.clone())
}

/// `p`, parameter `i`, as a field: a state property keeps its unrecognised values and its default.
fn field_param(lay: &Layout, i: usize, p: &Param) -> Param {
    let (Some(state), ParamKind::Variant(options)) = (&lay.state, &p.kind) else { return p.clone() };
    if state.index != i {
        return p.clone();
    }
    let kept = options.iter().filter(|o| state.kind(o) == StateKind::Other || p.default.as_str() == Some(o.as_str())).cloned().collect();
    Param { kind: ParamKind::Variant(kept), ..p.clone() }
}

fn state_kind_of(lay: &Layout, pairs: &BTreeMap<String, String>) -> StateKind {
    match &lay.state {
        Some(s) => pairs.get(&s.prop).map_or(StateKind::Rest, |v| s.kind(v)),
        None => StateKind::Rest,
    }
}

/// The variant attributes a variant with the property values `pairs` sets on the element, as
/// (attribute, value): a state property made only of known states sets none.
fn variant_attributes(c: &Component, lay: &Layout, pairs: &BTreeMap<String, String>) -> Vec<(String, String)> {
    let kind = state_kind_of(lay, pairs);
    c.params
        .iter()
        .zip(&lay.members)
        .enumerate()
        .filter(|(_, (p, _))| matches!(p.kind, ParamKind::Variant(_)))
        .filter_map(|(i, (p, m))| {
            let value = pairs.get(&p.prop)?;
            if lay.state.as_ref().is_some_and(|s| s.index == i) && kind != StateKind::Other {
                return None;
            }
            Some((m.attribute.clone()?, value.clone()))
        })
        .collect()
}

/// How to put the element in the state of one variant: its variant property values, the attributes
/// to set (a boolean one has an empty value) and the pseudo-class to force, if any.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StateRecipe {
    pub id: NodeId,
    pub props: BTreeMap<String, String>,
    pub attrs: BTreeMap<String, String>,
    pub pseudo: Option<&'static str>,
}

/// A recipe per variant of `c` (the component itself when it has none), in the set's order.
pub fn state_recipes(doc: &Document, c: &Component) -> Vec<StateRecipe> {
    let lay = layout(c);
    let variants: Vec<(NodeId, BTreeMap<String, String>)> = match c.set.and_then(|s| doc.node(s)) {
        Some(set) => set
            .children
            .iter()
            .filter_map(|v| doc.node(*v).filter(|n| matches!(n.kind, NodeKind::Component { .. })).map(|n| (*v, variant_props(&n.name))))
            .take(MAX_VARIANTS)
            .collect(),
        None => vec![(c.comp, BTreeMap::new())],
    };
    variants
        .into_iter()
        .map(|(id, props)| {
            let kind = state_kind_of(&lay, &props);
            let mut attrs: BTreeMap<String, String> = variant_attributes(c, &lay, &props).into_iter().collect();
            if let Some((_, _, attribute)) = lay.state.as_ref().and_then(|s| s.boolean(kind)) {
                attrs.insert(attribute.clone(), String::new());
            }
            let pseudo = match kind {
                StateKind::Hover => Some("hover"),
                StateKind::FocusVisible => Some("focus-visible"),
                StateKind::Active => Some("active"),
                _ => None,
            };
            StateRecipe { id, props, attrs, pseudo }
        })
        .collect()
}

/// Where a state comes in the cascade: a later one wins over an earlier one at the same specificity.
fn state_rank(kind: StateKind) -> u8 {
    match kind {
        StateKind::Rest | StateKind::Other => 0,
        StateKind::Hover => 1,
        StateKind::FocusVisible => 2,
        StateKind::Active => 3,
        StateKind::Loading => 4,
        StateKind::Disabled => 5,
    }
}

fn declared_property(line: &str) -> Option<&str> {
    line.split_once(':').map(|(property, _)| property)
}

/// The rules of the other variants of a set, on the host: what differs from the default variant,
/// or, for a pseudo-class state or an unrecognised one, from the variant with the same other values
/// at rest. Rules come in the order of the cascade (rest, hover, focus, pressed, loading, disabled),
/// and a later state repeats what an earlier one of the same variant sets: a hovered disabled
/// button looks disabled whatever the order of the variants. `main` are the default variant's
/// rules, `pass` those of a variant.
pub(crate) fn variant_rules(doc: &Document, c: &Component, main: &[Rule], pass: &dyn Fn(NodeId) -> Vec<Rule>, motion: &Motion) -> String {
    let lay = layout(c);
    let children = c.set.and_then(|s| doc.node(s)).map(|s| s.children.clone()).unwrap_or_default();
    let siblings: Vec<(NodeId, BTreeMap<String, String>)> = children
        .into_iter()
        .filter_map(|v| doc.node(v).filter(|n| matches!(n.kind, NodeKind::Component { .. })).map(|n| (v, variant_props(&n.name))))
        .take(MAX_VARIANTS)
        .collect();
    let kind_of = |pairs: &BTreeMap<String, String>| state_kind_of(&lay, pairs);
    let is_state = |i: usize| lay.state.as_ref().is_some_and(|s| s.index == i);
    let attributes = |pairs: &BTreeMap<String, String>| -> String {
        variant_attributes(c, &lay, pairs).iter().map(|(name, value)| format!("[{name}=\"{}\"]", css_string(value))).collect()
    };
    let same_axes = |a: &BTreeMap<String, String>, b: &BTreeMap<String, String>| {
        c.params
            .iter()
            .enumerate()
            .filter(|(i, p)| matches!(p.kind, ParamKind::Variant(_)) && !is_state(*i))
            .all(|(_, p)| a.get(&p.prop) == b.get(&p.prop))
    };
    let mut out = String::new();
    let mut written: Vec<(String, String)> = Vec::new();
    let mut touched: HashMap<(String, String), Vec<String>> = HashMap::new();
    let mut others: Vec<&(NodeId, BTreeMap<String, String>)> = siblings.iter().filter(|(v, _)| *v != c.comp).collect();
    others.sort_by_key(|(_, pairs)| state_rank(kind_of(pairs)));
    for (v, pairs) in others {
        let kind = kind_of(pairs);
        let pseudo = match kind {
            StateKind::Hover => ":hover".to_string(),
            StateKind::FocusVisible => ":focus-visible".to_string(),
            StateKind::Active => ":active".to_string(),
            StateKind::Disabled | StateKind::Loading => {
                lay.state.as_ref().and_then(|s| s.boolean(kind)).map(|(_, _, attribute)| format!("[{attribute}]")).unwrap_or_default()
            }
            StateKind::Rest | StateKind::Other => String::new(),
        };
        let selector = format!(":host({}{pseudo})", attributes(pairs));
        if selector == ":host()" {
            continue;
        }
        let at_rest = (kind != StateKind::Rest)
            .then(|| siblings.iter().find(|(w, wp)| w != v && same_axes(pairs, wp) && kind_of(wp) == StateKind::Rest))
            .flatten()
            .map(|(w, _)| *w)
            .filter(|w| *w != c.comp);
        let reference = at_rest.map(pass);
        let reference: &[Rule] = reference.as_deref().unwrap_or(main);
        let stacked = state_rank(kind) > 0;
        for (path, _, decls) in pass(*v) {
            let Some((_, class, base)) = main.iter().find(|(p, _, _)| *p == path) else { continue };
            let base = reference.iter().find(|(p, _, _)| *p == path).map_or(base, |(_, _, d)| d);
            let base_lines: Vec<&str> = base.lines().collect();
            let earlier = touched.entry((attributes(pairs), path.clone())).or_default();
            let lines: Vec<&str> = decls
                .lines()
                .filter(|l| !base_lines.contains(l) || (stacked && declared_property(l).is_some_and(|p| earlier.iter().any(|e| e == p))))
                .collect();
            if lines.is_empty() || written.contains(&(selector.clone(), class.clone())) {
                continue;
            }
            written.push((selector.clone(), class.clone()));
            if stacked {
                earlier.extend(lines.iter().filter_map(|l| declared_property(l)).map(str::to_string));
            }
            let transition = motion.state(*v, &path).into_iter();
            let lines = lines.iter().copied().chain(transition);
            out.push_str(&format!(
                "{selector} .{class} {{
{}}}
",
                lines
                    .map(|l| format!(
                        "  {l}
"
                    ))
                    .collect::<String>()
            ));
        }
    }
    out
}

/// One note per value of the state property that is no known interaction state.
fn state_notes(c: &Component, lay: &Layout) -> Vec<Note> {
    let Some(state) = lay.state.as_ref() else { return Vec::new() };
    state
        .values
        .iter()
        .filter(|(_, k)| *k == StateKind::Other)
        .map(|(value, _)| Note {
            code: "unrecognized-state",
            component: c.type_name.clone(),
            layer: None,
            message: format!("{} \"{value}\" is not a known interaction state: it stays a property", state.prop),
        })
        .collect()
}

/// A note when the set has more variants than get rules and recipes.
fn truncation_note(doc: &Document, c: &Component) -> Option<Note> {
    let set = c.set.and_then(|s| doc.node(s))?;
    let count = set.children.iter().filter(|v| doc.node(**v).is_some_and(|n| matches!(n.kind, NodeKind::Component { .. }))).count();
    (count > MAX_VARIANTS).then(|| Note {
        code: "variants-truncated",
        component: c.type_name.clone(),
        layer: None,
        message: format!("the set has {count} variants: only the first {MAX_VARIANTS} get rules and state recipes"),
    })
}

/// One note per variable whose Web code syntax the component does not follow.
fn syntax_notes(c: &Component, tokens: &Tokens) -> Vec<Note> {
    tokens
        .ignored
        .iter()
        .map(|i| Note {
            code: "code-syntax-ignored",
            component: c.type_name.clone(),
            layer: Some(i.layer.clone()),
            message: format!("{}: the Web code syntax {} is ignored, the component reads {}", i.variable, i.syntax, i.name),
        })
        .collect()
}

/// A component with a Focus state takes the keyboard focus, and not while it is disabled.
fn focus_methods(lay: &Layout) -> String {
    let Some(state) = lay.state.as_ref().filter(|s| s.has(StateKind::FocusVisible)) else { return String::new() };
    let Some((_, disabled, _)) = state.boolean(StateKind::Disabled) else {
        return "  connectedCallback() {\n    super.connectedCallback();\n    if (!this.hasAttribute(\"tabindex\")) this.setAttribute(\"tabindex\", \"0\");\n  }\n\n"
            .to_string();
    };
    format!(
        "  connectedCallback() {{\n    super.connectedCallback();\n    this.syncFocus();\n  }}\n\n  updated(changed: Map<PropertyKey, unknown>) {{\n    if (changed.has(\"{disabled}\")) this.syncFocus();\n  }}\n\n  private syncFocus() {{\n    this.setAttribute(\"tabindex\", this.{disabled} ? \"-1\" : \"0\");\n    this.setAttribute(\"aria-disabled\", String(this.{disabled}));\n  }}\n\n"
    )
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
    let (_, markup, css, tokens) = web_tokens(doc, c, CodeLang::Lit, opts);
    let hugs = doc.node(c.comp).is_some_and(|n| n.layout.is_some() && n.child_layout.sizing_h == figcraft_doc::Sizing::Hug);
    let display = if hugs { "inline-block" } else { "block" };
    let mut out =
        String::from("import { LitElement, css, html, nothing } from \"lit\";\nimport { customElement, property } from \"lit/decorators.js\";\n");
    out.push_str(&dependency_imports(doc, c, &tag, opts));
    out.push('\n');
    out.push_str(&format!("@customElement(\"{tag}\")\nexport class {class} extends LitElement {{\n"));
    let lay = layout(c);
    let mut fields: Vec<String> = c
        .params
        .iter()
        .zip(&lay.members)
        .enumerate()
        .filter(|(_, (_, m))| !m.hidden)
        .filter_map(|(i, (p, m))| field(&field_param(&lay, i, p), m))
        .collect();
    fields.extend(lay.extras.iter().filter_map(|e| field(&e.param, &e.member)));
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
    out.push_str("  `;\n\n");
    out.push_str(&focus_methods(&lay));
    out.push_str(&format!("  render() {{\n    return html`\n{markup}    `;\n  }}\n}}\n\n"));
    out.push_str(&format!("declare global {{\n  interface HTMLElementTagNameMap {{\n    \"{tag}\": {class};\n  }}\n}}\n"));
    let mut notes = state_notes(c, &lay);
    notes.extend(truncation_note(doc, c));
    notes.extend(syntax_notes(c, &tokens));
    notes.extend(tokens.unreproducible.iter().map(|u| Note {
        code: u.code,
        component: c.type_name.clone(),
        layer: u.layer.clone(),
        message: u.message.clone(),
    }));
    LitOutput { tag, class_name: class, ts: out, notes, tokens: tokens.names().map(str::to_string).collect() }
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
    m.use_lit(opts);
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
    let lay = layout(c);
    let (mut attrs, mut children, mut css) = (Vec::<String>::new(), String::new(), String::new());
    let mut attr = |text: String| {
        if !attrs.contains(&text) {
            attrs.push(text);
        }
    };
    for (i, ((p, m), (_, v))) in c.params.iter().zip(&lay.members).zip(values).enumerate() {
        if let Some(state) = lay.state.as_ref().filter(|s| s.index == i) {
            if let Some(a) = state_attribute(&lay, v) {
                attr(format!(" {a}"));
            }
            if m.hidden || v.as_str().is_some_and(|v| state.kind(v) != StateKind::Other) {
                continue;
            }
        }
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
                let Some(name) = m.attribute.as_deref() else { continue };
                attr(if v.as_bool() == Some(true) { format!(" {name}") } else { format!(" {name}=\"false\"") });
            }
            ParamKind::Text | ParamKind::Variant(_) => {
                let Some(name) = m.attribute.as_deref() else { continue };
                attr(format!(" {name}=\"{}\"", attribute_value(v.as_str().unwrap_or(""), template)));
            }
        }
    }
    let attrs = attrs.concat();
    let code = if children.is_empty() { format!("{pad}<{tag}{attrs}></{tag}>\n") } else { format!("{pad}<{tag}{attrs}>\n{children}{pad}</{tag}>\n") };
    (code, css)
}
