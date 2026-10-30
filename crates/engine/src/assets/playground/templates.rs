use std::collections::BTreeSet;

use crate::lit_export::{ExportPlan, PlannedComponent, Status};

const OPEN: &str = "<uigen-use";
const CLOSE: &str = "</uigen-use>";
const BASE_CSS: &str = include_str!("templates/base.css");
const MARKER_TEXT: &str = "Composant absent de la lib : ";
const GLOBAL_ATTRIBUTES: [&str; 5] = ["id", "class", "slot", "title", "hidden"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    Components,
    Dashboard,
    Mail,
    Chat,
    Finance,
    Crm,
}

impl Template {
    pub const ALL: [Self; 6] = [Self::Components, Self::Dashboard, Self::Mail, Self::Chat, Self::Finance, Self::Crm];

    pub fn name(self) -> &'static str {
        match self {
            Self::Components => "components",
            Self::Dashboard => "dashboard",
            Self::Mail => "mail",
            Self::Chat => "chat",
            Self::Finance => "finance",
            Self::Crm => "crm",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|template| template.name() == name)
    }

    fn source(self) -> &'static str {
        match self {
            Self::Components => "",
            Self::Dashboard => include_str!("templates/dashboard.html"),
            Self::Mail => include_str!("templates/mail.html"),
            Self::Chat => include_str!("templates/chat.html"),
            Self::Finance => include_str!("templates/finance.html"),
            Self::Crm => include_str!("templates/crm.html"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Composed {
    pub html: String,
    pub missing: Vec<String>,
    pub notes: Vec<String>,
}

pub fn compose(template: Template, plan: &ExportPlan) -> Composed {
    let library = Library::new(plan);
    let mut composer = Composer { library: &library, composed: Composed::default(), missing_keys: BTreeSet::new() };
    let body = match template {
        Template::Components => composer.components_page(plan),
        other => composer.expand(other.source()),
    };
    let mut composed = composer.composed;
    if !body.trim().is_empty() {
        composed.html = format!("<style>{BASE_CSS}</style>{body}");
    }
    composed
}

struct Entry<'a> {
    key: String,
    tag: &'a str,
    properties: BTreeSet<String>,
}

struct Library<'a> {
    entries: Vec<Entry<'a>>,
}

impl<'a> Library<'a> {
    fn new(plan: &'a ExportPlan) -> Self {
        let entries = plan
            .components
            .iter()
            .filter(|c| matches!(c.status, Status::Ok | Status::Dependency))
            .filter_map(|c| Some(Entry { key: key_of(&c.name), tag: valid_tag(c.tag.as_deref()?)?, properties: properties_of(c) }))
            .collect();
        Self { entries }
    }

    fn find(&self, names: &[&str]) -> Option<&Entry<'a>> {
        names.iter().find_map(|name| self.entries.iter().find(|e| e.key == key_of(name)))
    }
}

fn properties_of(component: &PlannedComponent) -> BTreeSet<String> {
    component.states.iter().flat_map(|state| state.props.keys().chain(state.attrs.keys())).map(|key| key.to_ascii_lowercase()).collect()
}

fn key_of(name: &str) -> String {
    name.chars().filter(|c| !c.is_whitespace() && *c != '-').flat_map(char::to_lowercase).collect()
}

fn valid_tag(tag: &str) -> Option<&str> {
    let usable = tag.contains('-')
        && tag.starts_with(|c: char| c.is_ascii_lowercase())
        && tag.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    usable.then_some(tag)
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

fn safe_attribute_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
        && !name.get(..2).is_some_and(|start| start.eq_ignore_ascii_case("on"))
}

fn safe_attribute_value(value: &str) -> bool {
    let lower = value.trim_start().to_ascii_lowercase();
    !(lower.contains("://") || lower.starts_with("//") || lower.starts_with("javascript:") || lower.contains("@import"))
}

fn is_global(name: &str) -> bool {
    GLOBAL_ATTRIBUTES.contains(&name) || name.starts_with("aria-") || name.starts_with("data-")
}

fn render_attribute(name: &str, value: &str) -> String {
    format!(" {}=\"{}\"", escape(name), escape(value))
}

struct Composer<'a> {
    library: &'a Library<'a>,
    composed: Composed,
    missing_keys: BTreeSet<String>,
}

impl Composer<'_> {
    fn note(&mut self, note: String) {
        if !self.composed.notes.contains(&note) {
            self.composed.notes.push(note);
        }
    }

    fn components_page(&mut self, plan: &ExportPlan) -> String {
        let mut sections = String::new();
        for component in plan.components.iter().filter(|c| c.status == Status::Ok) {
            let Some(tag) = component.tag.as_deref().and_then(valid_tag) else { continue };
            let instances = if component.states.is_empty() {
                format!("<{tag}></{tag}>")
            } else {
                component.states.iter().map(|state| self.instance(tag, state.attrs.iter())).collect()
            };
            sections.push_str(&format!(
                "<section class=\"pg-component\"><h2>{}</h2><div class=\"pg-variants\">{instances}</div></section>",
                escape(&component.name)
            ));
        }
        if sections.is_empty() {
            self.note("empty-components: the plan has no exportable component".to_string());
            return sections;
        }
        format!(
            "<style>.pg-components{{display:grid;gap:32px;padding:24px}}.pg-component{{display:grid;gap:12px}}.pg-component h2{{margin:0;font-size:1em}}.pg-variants{{display:flex;flex-wrap:wrap;align-items:flex-start;gap:16px}}</style><main class=\"pg-components\">{sections}</main>"
        )
    }

    fn instance<'s>(&mut self, tag: &str, attrs: impl Iterator<Item = (&'s String, &'s String)>) -> String {
        let mut open = String::new();
        for (name, value) in attrs {
            if safe_attribute_name(name) && safe_attribute_value(value) {
                open.push_str(&render_attribute(name, value));
            } else {
                self.note(format!("unsafe-attribute {name}"));
            }
        }
        format!("<{tag}{open}></{tag}>")
    }

    fn expand(&mut self, source: &str) -> String {
        let mut html = String::new();
        let mut rest = source;
        while let Some(at) = rest.find(OPEN) {
            html.push_str(&rest[..at]);
            let Some(slot) = Slot::read(&rest[at + OPEN.len()..]) else {
                self.note("malformed-template: an unreadable <uigen-use>".to_string());
                return html;
            };
            html.push_str(&self.slot(&slot));
            rest = slot.after;
        }
        html.push_str(rest);
        html
    }

    fn slot(&mut self, slot: &Slot) -> String {
        let Some(name) = slot.attr("name") else {
            self.note("malformed-template: a <uigen-use> without a name".to_string());
            return self.expand(slot.inner);
        };
        let alternatives = slot.attr("alt").map(|alt| alt.split('|').collect::<Vec<_>>()).unwrap_or_default();
        let names: Vec<&str> = std::iter::once(name).chain(alternatives).collect();
        let library = self.library;
        let Some(entry) = library.find(&names) else {
            self.mark_missing(name);
            let marker = format!("<div data-uigen-missing=\"{0}\" role=\"note\">{MARKER_TEXT}{0}</div>", escape(name));
            return marker + &self.expand(slot.inner);
        };
        let attributes = self.attributes(entry, &slot.attrs, name);
        let inner = self.expand(slot.inner);
        let tag = entry.tag;
        format!("<{tag}{attributes}>{inner}</{tag}>")
    }

    fn mark_missing(&mut self, name: &str) {
        if self.missing_keys.insert(key_of(name)) {
            self.composed.missing.push(name.to_string());
        }
    }

    fn attributes(&mut self, entry: &Entry, given: &[(String, String)], component: &str) -> String {
        let mut out = String::new();
        for (name, value) in given.iter().filter(|(n, _)| n != "name" && n != "alt") {
            if !safe_attribute_name(name) || !safe_attribute_value(value) {
                self.note(format!("unsafe-attribute {name} on {component}"));
            } else if is_global(name) || entry.properties.contains(name) {
                out.push_str(&render_attribute(name, value));
            } else {
                self.note(format!("unknown-prop {name} on {component}"));
            }
        }
        out
    }
}

struct Slot<'a> {
    attrs: Attributes,
    inner: &'a str,
    after: &'a str,
}

impl<'a> Slot<'a> {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    fn read(text: &'a str) -> Option<Self> {
        let (attrs, self_closing, body_at) = read_head(text)?;
        let body = text.get(body_at..)?;
        if self_closing {
            return Some(Self { attrs, inner: "", after: body });
        }
        let (inner_end, after_at) = find_close(body)?;
        Some(Self { attrs, inner: body.get(..inner_end)?, after: body.get(after_at..)? })
    }
}

type Attributes = Vec<(String, String)>;

fn read_head(text: &str) -> Option<(Attributes, bool, usize)> {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut attrs = Vec::new();
    loop {
        while bytes.get(i)?.is_ascii_whitespace() {
            i += 1;
        }
        match bytes.get(i)? {
            b'>' => return Some((attrs, false, i + 1)),
            b'/' if bytes.get(i + 1) == Some(&b'>') => return Some((attrs, true, i + 2)),
            _ => {}
        }
        let start = i;
        while bytes.get(i).is_some_and(|c| !c.is_ascii_whitespace() && !matches!(c, b'=' | b'>' | b'/')) {
            i += 1;
        }
        if i == start {
            return None;
        }
        let name = text.get(start..i)?.to_ascii_lowercase();
        let mut value = String::new();
        if bytes.get(i) == Some(&b'=') {
            i += 1;
            if bytes.get(i) != Some(&b'"') {
                return None;
            }
            i += 1;
            let from = i;
            while *bytes.get(i)? != b'"' {
                i += 1;
            }
            value = text.get(from..i)?.to_string();
            i += 1;
        }
        attrs.push((name, value));
    }
}

fn find_close(body: &str) -> Option<(usize, usize)> {
    let mut depth = 1;
    let mut at = 0;
    loop {
        let rest = body.get(at..)?;
        let next_open = rest.find(OPEN);
        let next_close = rest.find(CLOSE)?;
        match next_open {
            Some(open) if open < next_close => {
                depth += 1;
                at += open + OPEN.len();
            }
            _ => {
                depth -= 1;
                if depth == 0 {
                    return Some((at + next_close, at + next_close + CLOSE.len()));
                }
                at += next_close + CLOSE.len();
            }
        }
    }
}
