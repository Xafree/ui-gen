use std::collections::BTreeMap;

use figcraft_color::{Paint, PaintKind};

use super::css::{Tokens, css_decls};
use super::{CodeLang, Ctx, Document, LayoutMode, Node, NodeId, NodeKind, Param, Sizing, indent, num, with_v_if};
use crate::lit::LitOptions;

// ------------------------------------------------------------------ HTML and React markup

/// A word for the node's kind, naming layers whose name transliterates to nothing ("###" →
/// `.text`, `.frame`), instead of the anonymous `layer-`.
fn kind_word(n: &Node) -> &str {
    match &n.kind {
        NodeKind::Page { .. } => "page",
        NodeKind::Frame => "frame",
        NodeKind::Group => "group",
        NodeKind::Section { .. } => "section",
        NodeKind::Rectangle => "rectangle",
        NodeKind::Ellipse { .. } => "ellipse",
        NodeKind::Polygon { .. } => "polygon",
        NodeKind::Star { .. } => "star",
        NodeKind::Line => "line",
        NodeKind::Vector { .. } => "vector",
        NodeKind::Text(_) => "text",
        NodeKind::BooleanOp { .. } => "boolean",
        NodeKind::Component { .. } | NodeKind::ComponentSet { .. } => "component",
        NodeKind::Instance { .. } => "instance",
        NodeKind::Slice => "slice",
    }
}

/// A CSS class name from a layer name ("Card / Title" → "card-title"), unique in `used`. The name
/// is transliterated to ASCII first, so non-Latin names stay readable: 로그인 화면 →
/// `rogeuin-hwamyeon`, кнопка → `knopka`, über → `uber` (per-codepoint transliteration, the slug
/// libraries' way); a name left with nothing (symbols only) takes `kind`, and one starting with a
/// digit keeps the `layer-` prefix.
pub(crate) fn class_name(name: &str, kind: &str, used: &mut Vec<String>) -> String {
    let mut base = String::new();
    for c in deunicode::deunicode(name).chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            base.push(c);
        } else if !base.ends_with('-') && !base.is_empty() {
            base.push('-');
        }
    }
    // The 40-character cut lands on a word boundary when there is one past the first word.
    let base = if base.chars().count() > 40 {
        let cut: String = base.chars().take(40).collect();
        match cut.rfind('-') {
            Some(i) if i > 0 => cut[..i].to_string(),
            _ => cut,
        }
    } else {
        base
    };
    let base = base.trim_end_matches('-');
    let base = if base.is_empty() {
        kind.to_string()
    } else if base.starts_with(|c: char| c.is_ascii_digit()) {
        format!("layer-{base}")
    } else {
        base.to_string()
    };
    let name = (1..).map(|k| if k == 1 { base.clone() } else { format!("{base}-{k}") }).find(|n| !used.contains(n)).unwrap_or(base);
    used.push(name.clone());
    name
}

pub(crate) fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('\n', "<br>")
}

/// Text as JSX children: HTML escapes, braces as expressions, line breaks as `<br />`.
fn jsx_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().take(10_000) {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '{' => out.push_str("{\"{\"}"),
            '}' => out.push_str("{\"}\"}"),
            '\n' => out.push_str("<br />"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// The whole tree under `id` as nested HTML with a stylesheet: auto layout frames become
/// flexboxes, other children are placed absolutely in their parent, text becomes paragraphs.
pub(super) fn html(doc: &Document, id: NodeId) -> String {
    let ctx = Ctx::new(doc, id);
    let mut out = Markup::new(&ctx, CodeLang::Html);
    out.node(id, None, 0, "");
    format!("{}\n<style>\n{}{}</style>\n", out.markup, out.tokens.root_rule(), out.styles)
}

/// Markup being built for a layer tree: HTML (`class`), React JSX (`className`), Vue
/// (`{{ }}` interpolation, `v-if`, `:attr` bindings) or Leptos `view!` markup (text as Rust
/// string literals); the last three with component parameters and instances as calls. With its
/// stylesheet.
pub(crate) struct Markup<'c, 'a> {
    ctx: &'c Ctx<'a>,
    /// Html, React, Vue or Leptos.
    lang: CodeLang,
    used: Vec<String>,
    pub markup: String,
    pub styles: String,
    pub tokens: Tokens,
    /// The root's class (default: from its name) and extra JSX attributes.
    pub root_class: Option<String>,
    pub root_attrs: String,
    /// A Vue `v-if` guard for the element about to be written: Vue templates can't wrap an
    /// element in an expression, so a boolean parameter's layer carries the condition itself.
    v_if: Option<String>,
    /// A prefix for every generated class name (slot content joins the caller's stylesheet, where
    /// two slots' layer names may repeat).
    pub class_prefix: Option<String>,
    /// Each layer's path in the tree (names with sibling counts), class and declarations, so
    /// variants can be compared layer by layer.
    pub rules: Vec<(String, String, String)>,
    /// Instances as their own (overridden) layers instead of component calls, except those
    /// connected to code (page code).
    pub inline: bool,
    /// Auto layout frames and text as fluid boxes (`width: 100%` up to their width) for pages.
    pub fluid: bool,
    /// With `fluid`, other layers at least this share of the root's width are fluid too (a
    /// divider, a banner image); smaller ones (icons) keep their size. Default: none.
    pub fluid_leaves: f64,
    /// Lit: the tag prefix of the components it calls.
    pub lit: LitOptions,
    /// Lit: the markup is plain HTML, not the content of a `html` template literal.
    pub lit_plain: bool,
    /// Each layer's class, in the order they are written.
    pub ids: Vec<(NodeId, String)>,
    /// Lit: declarations added to the rule of the layer at a path (transitions, effects).
    pub lit_motion: BTreeMap<String, String>,
}

/// How `n` sits in its parent: in the flex flow, or placed at its position.
fn placement_decls(n: &Node, parent: Option<&Node>, root: bool, in_flow: bool) -> String {
    let mut decls = String::new();
    if root {
        decls.push_str("position: relative;\n");
    } else if in_flow {
        let vertical = parent.and_then(|p| p.layout.as_ref()).is_some_and(|l| l.mode == LayoutMode::Vertical);
        let (main, cross) =
            if vertical { (n.child_layout.sizing_v, n.child_layout.sizing_h) } else { (n.child_layout.sizing_h, n.child_layout.sizing_v) };
        if main == Sizing::Fill {
            decls.push_str("flex: 1 1 0;\n");
        }
        if cross == Sizing::Fill {
            decls.push_str("align-self: stretch;\n");
        }
    } else {
        let c = n.transform.as_coeffs();
        decls.push_str(&format!("position: absolute;\nleft: {}px;\ntop: {}px;\n", num(c[4]), num(c[5])));
        let rot = figcraft_geom::rotation_degrees(n.transform);
        if rot.abs() > 0.01 {
            decls.push_str(&format!("transform: rotate({}deg);\ntransform-origin: 0 0;\n", num(-rot)));
        }
    }
    decls
}

impl<'c, 'a> Markup<'c, 'a> {
    pub fn new(ctx: &'c Ctx<'a>, lang: CodeLang) -> Self {
        let lit = LitOptions::default();
        let mut tokens = Tokens::default();
        if lang == CodeLang::Lit {
            tokens.lit_prefix = Some(lit.token_prefix.clone());
        }
        Markup {
            ctx,
            lang,
            used: Vec::new(),
            markup: String::new(),
            styles: String::new(),
            tokens,
            root_class: None,
            root_attrs: String::new(),
            v_if: None,
            class_prefix: None,
            rules: Vec::new(),
            inline: false,
            fluid: false,
            fluid_leaves: f64::INFINITY,
            lit,
            lit_plain: false,
            ids: Vec::new(),
            lit_motion: BTreeMap::new(),
        }
    }

    /// The Lit components this markup calls, and (in Lit) its tokens, follow `opts`.
    pub fn use_lit(&mut self, opts: &LitOptions) {
        self.lit = opts.clone();
        if self.lang == CodeLang::Lit {
            self.tokens.lit_prefix = Some(opts.token_prefix.clone());
        }
    }

    /// JSX, Vue and Lit templates indent by two spaces, HTML and Leptos by four.
    fn pad(&self, depth: usize) -> String {
        if matches!(self.lang, CodeLang::React | CodeLang::Vue | CodeLang::Lit) { "  ".repeat(depth) } else { indent(depth) }
    }

    /// Component code (React or Leptos): parameters and instances as calls.
    fn code(&self) -> bool {
        self.lang != CodeLang::Html
    }

    /// A parameter's name in the generated code.
    fn ident(&self, p: &Param) -> String {
        match (self.lang, self.ctx.comp) {
            (CodeLang::Leptos, _) => crate::component_codegen::rust_ident(&p.ident),
            (CodeLang::Lit, Some(c)) => crate::lit::member(c, p).ident,
            _ => p.ident.clone(),
        }
    }

    /// A layer, shown only while its boolean parameter is on (component code).
    pub fn node(&mut self, id: NodeId, parent: Option<&Node>, depth: usize, path: &str) {
        let Some(n) = self.ctx.doc.node(id) else { return };
        if self.code()
            && id != self.ctx.root
            && let Some(p) = self.ctx.param(n, "visible")
        {
            // A Vue template holds no arbitrary expressions, so the guard rides on the element.
            if self.lang == CodeLang::Vue {
                self.v_if = Some(self.ident(p));
                self.element(n, parent, depth, path);
                self.v_if = None;
                return;
            }
            let (pad, p) = (self.pad(depth), self.ident(p));
            let (open, close) = match self.lang {
                CodeLang::Leptos => (format!("{{{p}.then(|| view! {{"), "})}"),
                CodeLang::Lit => (format!("${{this.{p} ? html`"), "` : nothing}"),
                _ => (format!("{{{p} && ("), ")}"),
            };
            self.markup.push_str(&format!("{pad}{open}\n"));
            self.element(n, parent, depth + 1, path);
            self.markup.push_str(&format!("{pad}{close}\n"));
            return;
        }
        self.element(n, parent, depth, path);
    }

    fn rule(&mut self, path: &str, class: &str, decls: String) {
        let decls = match self.lit_motion.get(path) {
            Some(extra) => merge_decls(&decls, extra),
            None => decls,
        };
        self.styles.push_str(&format!(".{class} {{\n"));
        for line in decls.lines() {
            self.styles.push_str(&format!("  {line}\n"));
        }
        self.styles.push_str("}\n");
        self.rules.push((path.to_string(), class.to_string(), decls));
    }

    /// An instance as a call to its component (see `element`); true when it was written.
    fn instance_call(&mut self, n: &Node, depth: usize, path: &str, class: &str, decls: &str, root: bool) -> bool {
        let pad = self.pad(depth);
        let attr = if self.lang == CodeLang::React { "className" } else { "class" };
        let connected = self.inline && crate::code_connect::link(self.ctx.doc, n.id, CodeLang::React).is_some();
        if self.code() && (!self.inline || connected) && !root && matches!(n.kind, NodeKind::Instance { .. }) {
            let at = depth + usize::from(!decls.is_empty());
            let run = if self.lang == CodeLang::Leptos { ".run()" } else { "" };
            let (inner, css) = match self.ctx.param(n, "mainComponent") {
                Some(p) if self.lang == CodeLang::Vue => (Some(format!("{}<component :is=\"{}\" />\n", self.pad(at), self.ident(p))), String::new()),
                Some(p) if self.lang == CodeLang::Lit => (Some(crate::lit::swap_slot(self.ctx, p, at, &self.lit)), String::new()),
                Some(p) => (Some(format!("{}{{{}{run}}}\n", self.pad(at), self.ident(p))), String::new()),
                None if self.lang == CodeLang::Lit => {
                    crate::component_codegen::usage_in(self.ctx.doc, n.id, self.lang, at, &self.lit, !self.lit_plain)
                        .map_or((None, String::new()), |(code, css)| (Some(code), css))
                }
                None => self.ctx.call(n, self.lang, at).map_or((None, String::new()), |(code, css)| (Some(code), css)),
            };
            self.styles.push_str(&css);
            if let Some(inner) = inner {
                // A Vue guard set by `node` rides on the call when there is no wrapper.
                let guard = if self.lang == CodeLang::Vue { self.v_if.take() } else { None };
                if decls.is_empty() {
                    self.markup.push_str(&with_v_if(&inner, guard.as_deref()));
                } else {
                    let guard = guard.map(|g| format!(" v-if=\"{g}\"")).unwrap_or_default();
                    self.markup.push_str(&format!("{pad}<div {attr}=\"{class}\"{guard}>\n{inner}{pad}</div>\n"));
                    self.rule(path, class, decls.to_string());
                }
                return true;
            }
        }
        false
    }

    /// Lit: a vector layer as the inline `<svg>` of its export, placed by its own rule; false for
    /// any other layer, or a vector that doesn't export (it stays a box, with a note).
    fn inline_vector(&mut self, n: &Node, depth: usize, path: &str, class: &str, placement: &str) -> bool {
        if self.lang != CodeLang::Lit || !matches!(n.kind, NodeKind::Vector { .. }) {
            return false;
        }
        let doc = self.ctx.doc;
        let mut drawn = doc.in_srgb();
        if has_image_paint(n) {
            self.tokens.note("image-fill", Some(&n.name), "image and pattern fills are not exported: the layer is drawn without them");
            if let Some(copy) = drawn.to_mut().node_mut(n.id) {
                copy.fills.retain(|p| !is_image_paint(p));
                copy.strokes.retain(|p| !is_image_paint(p));
            }
        }
        for field in ["fill", "stroke"] {
            if let Some(variable) = crate::devinspect::bound_variable(doc, n, field) {
                let message =
                    format!("the {field} is bound to {}: the inline SVG keeps the colour it has today and does not follow the token", variable.name);
                self.tokens.note("vector-token-frozen", Some(&n.name), &message);
            }
        }
        let options = figcraft_svg::SvgOptions { id_prefix: format!("{class}-"), ..Default::default() };
        let Ok(svg) = figcraft_svg::try_export_svg(&drawn, &[n.id], &options) else {
            self.tokens.note("vector-fallback", Some(&n.name), "the vector could not be exported as SVG: the layer is drawn as a plain box");
            return false;
        };
        let mut own_tokens = Tokens::lit(&self.lit.token_prefix);
        let own = css_decls(doc, n, &mut own_tokens, (false, false));
        for u in &own_tokens.unreproducible {
            self.tokens.note(u.code, u.layer.as_deref(), &u.message);
        }
        let kept = own.lines().filter(|l| l.starts_with("opacity:") || l.starts_with("mix-blend-mode:"));
        let placed = placement.lines().filter(|l| !l.starts_with("transform"));
        let decls: String = placed.chain(kept).map(|l| format!("{l}\n")).collect();
        self.rule(path, class, decls);
        let svg = svg.trim_end().replacen("<svg ", &format!("<svg class=\"{class}\" "), 1);
        let svg = if self.lit_plain { svg } else { crate::lit::template_text(&svg) };
        self.markup.push_str(&format!("{}{svg}\n", self.pad(depth)));
        true
    }

    fn text_body(&self, n: &Node, t: &figcraft_doc::TextData) -> String {
        match (self.ctx.param(n, "characters").filter(|_| self.code()), self.lang) {
            (Some(p), CodeLang::Leptos) => format!("{{{}.clone()}}", self.ident(p)),
            (Some(p), CodeLang::Vue) => format!("{{{{ {} }}}}", self.ident(p)),
            (Some(p), CodeLang::Lit) => format!("${{this.{}}}", self.ident(p)),
            (Some(p), _) => format!("{{{}}}", p.ident),
            (None, CodeLang::React) => jsx_text(&t.characters),
            // `view!` takes text as string literals and void elements self-closed.
            (None, CodeLang::Leptos) => {
                t.characters.chars().take(10_000).collect::<String>().lines().map(|l| format!("{l:?}")).collect::<Vec<_>>().join("<br/>")
            }
            // A Vue template reads `{{ }}` as interpolation, so braces are escaped.
            (None, CodeLang::Vue) => html_escape(&t.characters).replace('{', "&#123;").replace('}', "&#125;"),
            (None, CodeLang::Lit) if self.lit_plain => html_escape(&t.characters),
            (None, CodeLang::Lit) => crate::lit::text(&t.characters),
            (None, _) => html_escape(&t.characters),
        }
    }

    /// Lit: the default `<slot>` line closing a component root that can hold content, else nothing.
    fn default_slot(&self, n: &Node, root: bool, named: bool, depth: usize) -> String {
        let holds = self.lang == CodeLang::Lit && root && !named && self.ctx.comp.is_some() && crate::lit::takes_content(self.ctx.doc, n);
        if holds { format!("{}<slot></slot>\n", self.pad(depth)) } else { String::new() }
    }

    fn element(&mut self, n: &Node, parent: Option<&Node>, depth: usize, path: &str) {
        let doc = self.ctx.doc;
        let root = n.id == self.ctx.root;
        let class = match self.root_class.take().filter(|_| root) {
            Some(c) => {
                self.used.push(c.clone());
                c
            }
            None => {
                let class = class_name(&n.name, kind_word(n), &mut self.used);
                match &self.class_prefix {
                    Some(prefix) => format!("{prefix}{class}"),
                    None => class,
                }
            }
        };
        self.ids.push((n.id, class.clone()));
        let pad = self.pad(depth);
        let attr = if self.lang == CodeLang::React { "className" } else { "class" };
        // How it sits in its parent: in the flex flow, or placed at its position.
        let in_flow = parent.is_some_and(|p| p.layout.is_some()) && !n.child_layout.absolute;
        let mut decls = placement_decls(n, parent, root, in_flow);
        // Nested instances are calls (component code), in a wrapper that places them when needed.
        // Page code inlines instances, except those connected to code (codeConnect.set): their
        // snippet, e.g. a design-system `<Button>`.
        if self.instance_call(n, depth, path, &class, &decls, root) || self.inline_vector(n, depth, path, &class, &decls) {
            return;
        }
        let hugs = n.layout.is_some() || matches!(n.kind, NodeKind::Text(_));
        let hug = (hugs && n.child_layout.sizing_h == Sizing::Hug, hugs && n.child_layout.sizing_v == Sizing::Hug);
        let mut own = css_decls(doc, n, &mut self.tokens, hug);
        let page = self.ctx.doc.node(self.ctx.root).map_or(f64::INFINITY, |r| r.width);
        if self.fluid && (n.layout.is_some() || matches!(n.kind, NodeKind::Text(_)) || (!root && n.width >= page * self.fluid_leaves)) {
            // Full width, or sized by the flex row (`flex: 1`): no cap needed.
            own = fluid(&own, n.width >= page - 0.5 || decls.contains("flex: 1 1 0;"), n.layout.is_some());
        }
        if n.layout.is_none() && !n.children.is_empty() && !root && in_flow {
            own.push_str("position: relative;\n");
        }
        decls.push_str(&own);
        self.rule(path, &class, decls);
        // The root's variant attributes, and a Vue `v-if` guard, ride on the open tag.
        let mut attrs = if root { self.root_attrs.clone() } else { String::new() };
        if self.lang == CodeLang::Vue
            && let Some(guard) = self.v_if.take()
        {
            attrs.push_str(&format!(" v-if=\"{guard}\""));
        }
        let open = |tag: &str| format!("{pad}<{tag} {attr}=\"{class}\"{attrs}>");
        match &n.kind {
            NodeKind::Text(t) => {
                let body = self.text_body(n, t);
                self.markup.push_str(&format!("{}{body}</p>\n", open("p")));
            }
            _ => {
                let kids = self.ctx.kids(n);
                let slot = self.ctx.param(n, "slot").filter(|_| self.code());
                let vue = self.lang == CodeLang::Vue;
                let leptos = self.lang == CodeLang::Leptos;
                let lit = self.lang == CodeLang::Lit;
                let open = open("div");
                let default_slot = self.default_slot(n, root, slot.is_some(), depth + 1);
                let slot_ident = slot.map(|p| match self.ctx.comp {
                    Some(c) if lit => crate::lit::slot_name(c, p),
                    Some(c) => crate::component_codegen::slot_name(c, p),
                    None => class_name(&p.ident, "slot", &mut Vec::new()),
                });
                if let Some(p) = slot.filter(|_| kids.is_empty()) {
                    let content = if leptos {
                        format!("{{{}}}.as_ref().map(|c| c.run())", self.ident(p))
                    } else if vue {
                        // Vue's own slot element, named so an instance's content lands in the right one.
                        match slot_ident.clone() {
                            Some(name) => format!("<slot name=\"{name}\" />"),
                            None => "<slot />".into(),
                        }
                    } else if lit {
                        format!("<slot name=\"{}\"></slot>", slot_ident.clone().unwrap_or_default())
                    } else {
                        format!("{{{}}}", p.ident)
                    };
                    self.markup.push_str(&format!("{open}{content}</div>\n"));
                } else if kids.is_empty() || self.ctx.too_deep(depth) {
                    self.markup.push_str(&format!("{open}{}</div>\n", default_slot.trim()));
                } else {
                    self.markup.push_str(&format!("{open}\n"));
                    // A slot shows the instance's content, or else the main's.
                    let (p1, p2) = (self.pad(depth + 1), self.pad(depth + 2));
                    let inner = match slot {
                        Some(_) if vue || lit => {
                            let name = slot_ident.clone().map(|n| format!(" name=\"{n}\"")).unwrap_or_default();
                            self.markup.push_str(&format!("{p1}<slot{name}>\n"));
                            depth + 2
                        }
                        Some(p) if leptos => {
                            self.markup.push_str(&format!("{p1}{{match &{} {{ Some(c) => c.run(), None => view! {{\n", self.ident(p)));
                            depth + 2
                        }
                        Some(p) => {
                            self.markup.push_str(&format!("{p1}{{{} ?? (\n{p2}<>\n", p.ident));
                            depth + 3
                        }
                        None => depth + 1,
                    };
                    let mut seen: Vec<&str> = Vec::new();
                    // Bottom layer first: later elements paint on top, like the layer order.
                    for k in kids {
                        let Some(kn) = doc.node(k) else { continue };
                        let count = seen.iter().filter(|s| **s == kn.name).count();
                        seen.push(&kn.name);
                        self.node(k, Some(n), inner, &format!("{path}/{}#{count}", kn.name));
                    }
                    match slot {
                        Some(_) if vue || lit => self.markup.push_str(&format!("{p1}</slot>\n")),
                        Some(_) if leptos => self.markup.push_str(&format!("{p1}}}.into_any() }}}}\n")),
                        Some(_) => self.markup.push_str(&format!("{p2}</>\n{p1})}}\n")),
                        None => {}
                    }
                    self.markup.push_str(&default_slot);
                    self.markup.push_str(&format!("{pad}</div>\n"));
                }
            }
        }
    }
}

fn is_image_paint(p: &Paint) -> bool {
    p.visible && matches!(p.kind, PaintKind::Image(_) | PaintKind::Pattern(_))
}

fn has_image_paint(n: &Node) -> bool {
    n.fills.iter().chain(&n.strokes).any(is_image_paint)
}

/// `decls` without the declarations `extra` sets again, then `extra`.
fn merge_decls(decls: &str, extra: &str) -> String {
    let property = |line: &str| line.split_once(':').map(|(p, _)| p.to_string());
    let replaced: Vec<String> = extra.lines().filter_map(property).collect();
    let kept = decls.lines().filter(|l| !property(l).is_some_and(|p| replaced.contains(&p)));
    kept.chain(extra.lines()).map(|l| format!("{l}\n")).collect()
}

/// `decls` for a box that follows the page: a fixed width becomes `width: 100%` capped at that
/// width (or just the full width for boxes as wide as the page), and a frame's fixed height a
/// minimum, so content can wrap and grow.
// ponytail: width-based heuristic; fixed-size leaves (icons, images, shapes) keep their pixels.
fn fluid(decls: &str, full: bool, frame: bool) -> String {
    let mut out = String::with_capacity(decls.len() + 32);
    for line in decls.lines() {
        if let Some(w) = line.strip_prefix("width: ") {
            out.push_str("width: 100%;\n");
            if !full {
                out.push_str(&format!("max-width: {w}\n"));
            }
        } else if let Some(h) = line.strip_prefix("height: ").filter(|_| frame) {
            out.push_str(&format!("min-height: {h}\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}
