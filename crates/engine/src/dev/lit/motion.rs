//! Lit motion: the prototype transitions between a set's variants become CSS transitions, ripple,
//! spinner and pulse layers become keyframe animations, reduced motion switches both off, and what
//! CSS can't play is noted instead of invented.

use std::collections::{BTreeMap, HashMap};

use figcraft_doc::proto::{Action, Easing, Interaction, Transition, Trigger};
use figcraft_doc::{Document, Node, NodeId, NodeKind};

use super::{Layout, LitOptions, Rule, StateKind, fold, layout, tag_for};
use crate::codegen::Unreproducible;
use crate::component_codegen::{Component, ParamKind};
use crate::components::variant_props;

/// Most interactions read from one variant.
const MAX_INTERACTIONS: usize = 32;

/// Longest a ripple runs when no interaction says (ms).
const DEFAULT_RIPPLE_MS: u32 = 600;

/// Properties a CSS transition can't animate.
const DISCRETE: &[&str] = &[
    "display",
    "position",
    "flex-direction",
    "flex-wrap",
    "justify-content",
    "align-items",
    "align-content",
    "align-self",
    "font-family",
    "font-style",
    "text-align",
    "text-transform",
    "transform-origin",
    "mix-blend-mode",
];

const REDUCED_MOTION: &str = "@media (prefers-reduced-motion: reduce) {\n  *,\n  *::before,\n  *::after {\n    transition: none !important;\n    animation: none !important;\n  }\n}\n";

const SPRING_POINTS: u32 = 32;

/// A number written short: three decimals at most, never `-0`.
fn short(v: f64) -> String {
    let rounded = if v.is_finite() { (v * 1000.0).round() / 1000.0 } else { 0.0 };
    (rounded + 0.0).to_string()
}

/// The CSS timing function of `easing`: a keyword, a `cubic-bezier`, or for a spring a `linear()`
/// sampled from its curve.
pub fn easing_css(easing: Easing) -> String {
    match easing {
        Easing::Linear => "linear".into(),
        Easing::EaseIn => "cubic-bezier(0.32, 0, 0.67, 0)".into(),
        Easing::EaseOut => "cubic-bezier(0.33, 1, 0.68, 1)".into(),
        Easing::EaseInOut => "cubic-bezier(0.65, 0, 0.35, 1)".into(),
        Easing::EaseInBack => "cubic-bezier(0.36, 0, 0.66, -0.56)".into(),
        Easing::EaseOutBack => "cubic-bezier(0.34, 1.56, 0.64, 1)".into(),
        Easing::EaseInOutBack => "cubic-bezier(0.68, -0.6, 0.32, 1.6)".into(),
        Easing::CustomBezier { x1, y1, x2, y2 } => {
            format!("cubic-bezier({}, {}, {}, {})", short(x1.clamp(0.0, 1.0)), short(y1), short(x2.clamp(0.0, 1.0)), short(y2))
        }
        _ => {
            let points: Vec<String> = (0..=SPRING_POINTS).map(|i| short(easing.eval(f64::from(i) / f64::from(SPRING_POINTS)))).collect();
            format!("linear({})", points.join(", "))
        }
    }
}

#[derive(Clone, Copy)]
struct Timing {
    ms: u32,
    easing: Easing,
    delay_ms: u32,
}

impl Timing {
    fn of(it: &Interaction) -> Timing {
        let ms = (it.easing.seconds(it.duration_ms) * 1000.0).round().clamp(0.0, 600_000.0) as u32;
        Timing { ms, easing: it.easing, delay_ms: (it.delay_secs() * 1000.0).round().clamp(0.0, 600_000.0) as u32 }
    }

    /// `background 200ms ease-out 150ms`: the delay comes after the curve.
    fn entry(self, property: &str) -> String {
        let delay = if self.delay_ms > 0 { format!(" {}ms", self.delay_ms) } else { String::new() };
        format!("{property} {}ms {}{delay}", self.ms, easing_css(self.easing))
    }
}

/// A change between two variants that a pointer gesture triggers.
struct Edge {
    from: NodeId,
    to: NodeId,
    /// Into a pseudo-class state (hover, focus, pressed), not back to rest.
    entering: bool,
    kind: StateKind,
    timing: Timing,
}

/// What the Lit output adds for a component's motion.
#[derive(Default)]
pub(crate) struct Motion {
    /// Declarations added to the rule of the layer at a path.
    pub base: BTreeMap<String, String>,
    /// Rules, keyframes and the reduced-motion block, after the others.
    pub tail: String,
    pub notes: Vec<Unreproducible>,
    states: BTreeMap<(NodeId, String), String>,
}

impl Motion {
    /// The transition the rule of variant `variant` sets on the layer at `path`.
    pub fn state(&self, variant: NodeId, path: &str) -> Option<&str> {
        self.states.get(&(variant, path.to_string())).map(String::as_str)
    }

    fn declare(&mut self, path: &str, decls: &str) {
        let slot = self.base.entry(path.to_string()).or_default();
        slot.push_str(decls);
        slot.push('\n');
    }

    fn note(&mut self, code: &'static str, layer: Option<&str>, message: &str) {
        let note = Unreproducible { code, layer: layer.map(str::to_string), message: message.to_string() };
        if !self.notes.contains(&note) {
            self.notes.push(note);
        }
    }
}

/// The motion of component `c`: `main` and `ids` are the rules and the layers' classes of its
/// default variant, `rules_of` those of any variant.
pub(crate) fn plan(
    doc: &Document,
    c: &Component,
    lit: &LitOptions,
    main: &[Rule],
    ids: &[(NodeId, String)],
    rules_of: &dyn Fn(NodeId) -> Vec<Rule>,
) -> Motion {
    let lay = layout(c);
    let mut motion = Motion::default();
    let edges = edges(doc, &lay, &variants_of(doc, c), &mut motion);
    transitions(&edges, rules_of, &mut motion);
    let scene = Scene { doc, c, tag: tag_for(doc, c, lit), lay: &lay, edges: &edges, main, ids };
    let effects = scene.effects(&mut motion);
    if effects || !motion.states.is_empty() || motion.base.values().any(|d| d.contains("transition:")) {
        motion.tail.push_str(REDUCED_MOTION);
    }
    motion
}

fn variants_of(doc: &Document, c: &Component) -> Vec<NodeId> {
    let Some(set) = c.set.and_then(|s| doc.node(s)) else { return Vec::new() };
    let is_variant = |id: &&NodeId| doc.node(**id).is_some_and(|n| matches!(n.kind, NodeKind::Component { .. }));
    set.children.iter().filter(is_variant).take(64).copied().collect()
}

fn kind_of(doc: &Document, lay: &Layout, variant: NodeId) -> StateKind {
    let (Some(state), Some(n)) = (&lay.state, doc.node(variant)) else { return StateKind::Rest };
    variant_props(&n.name).get(&state.prop).map_or(StateKind::Rest, |value| state.kind(value))
}

fn is_pseudo(kind: StateKind) -> bool {
    matches!(kind, StateKind::Hover | StateKind::FocusVisible | StateKind::Active)
}

fn is_pointer_trigger(trigger: &Trigger) -> bool {
    matches!(trigger, Trigger::OnHover | Trigger::OnPress | Trigger::MouseEnter | Trigger::MouseLeave | Trigger::MouseDown | Trigger::MouseUp)
}

fn transition_name(transition: &Transition) -> &'static str {
    match transition {
        Transition::Instant => "instant",
        Transition::Dissolve => "dissolve",
        Transition::SmartAnimate => "smart animate",
        Transition::MoveIn { .. } => "move in",
        Transition::MoveOut { .. } => "move out",
        Transition::Push { .. } => "push",
        Transition::SlideIn { .. } => "slide in",
        Transition::SlideOut { .. } => "slide out",
    }
}

/// The pointer-driven changes between the variants; what can't be played is noted.
fn edges(doc: &Document, lay: &Layout, variants: &[NodeId], motion: &mut Motion) -> Vec<Edge> {
    let mut edges = Vec::new();
    for &from in variants {
        let Some(n) = doc.node(from) else { continue };
        for it in n.interactions.iter().take(MAX_INTERACTIONS).filter(|i| is_pointer_trigger(&i.trigger)) {
            for action in &it.actions {
                match action {
                    Action::ChangeTo { dest } if variants.contains(dest) => edges.extend(edge(doc, lay, (from, *dest), it, motion)),
                    _ => motion.note("unsupported-action", None, "only a change to another variant is exported: this action is left out"),
                }
            }
        }
    }
    edges
}

fn edge(doc: &Document, lay: &Layout, (from, to): (NodeId, NodeId), it: &Interaction, motion: &mut Motion) -> Option<Edge> {
    match it.transition {
        Transition::SmartAnimate | Transition::Dissolve => {}
        Transition::Instant => return None,
        other => {
            let message = format!("a {} transition has no CSS equivalent: the state changes without animation", transition_name(&other));
            motion.note("unsupported-transition", None, &message);
            return None;
        }
    }
    let (before, kind) = (kind_of(doc, lay, from), kind_of(doc, lay, to));
    let entering = is_pseudo(kind);
    (entering || (kind == StateKind::Rest && is_pseudo(before))).then(|| Edge { from, to, entering, kind, timing: Timing::of(it) })
}

type Entries = Vec<(String, String)>;

fn merge(into: &mut Entries, entries: &Entries) {
    for (property, entry) in entries {
        if !into.iter().any(|(p, _)| p == property) {
            into.push((property.clone(), entry.clone()));
        }
    }
}

fn transition_line(entries: &Entries) -> String {
    format!("transition: {};", entries.iter().map(|(_, e)| e.as_str()).collect::<Vec<_>>().join(", "))
}

/// Per layer, the properties that differ between `before` and `after` with their transition entry.
fn changed(before: &[Rule], after: &[Rule], timing: Timing) -> Vec<(String, Entries)> {
    after
        .iter()
        .filter_map(|(path, _, decls)| {
            let (_, _, was) = before.iter().find(|(p, _, _)| p == path)?;
            let was: Vec<&str> = was.lines().collect();
            let mut entries = Entries::new();
            for line in decls.lines().filter(|l| !was.contains(l)) {
                let Some((property, _)) = line.split_once(':') else { continue };
                if !DISCRETE.contains(&property) && !entries.iter().any(|(p, _)| p == property) {
                    entries.push((property.to_string(), timing.entry(property)));
                }
            }
            (!entries.is_empty()).then(|| (path.clone(), entries))
        })
        .collect()
}

/// The coming back to rest sets the base rule's transition, or, with none, the going in does; a
/// state keeps its own when it differs from the base.
fn transitions(edges: &[Edge], rules_of: &dyn Fn(NodeId) -> Vec<Rule>, motion: &mut Motion) {
    let mut cache: HashMap<NodeId, Vec<Rule>> = HashMap::new();
    let mut rules = |id: NodeId| cache.entry(id).or_insert_with(|| rules_of(id)).clone();
    let leaving = edges.iter().any(|e| !e.entering);
    let mut base: BTreeMap<String, Entries> = BTreeMap::new();
    let mut states: BTreeMap<(NodeId, String), Entries> = BTreeMap::new();
    for e in edges {
        for (path, entries) in changed(&rules(e.from), &rules(e.to), e.timing) {
            if e.entering {
                merge(states.entry((e.to, path.clone())).or_default(), &entries);
            }
            if e.entering != leaving {
                merge(base.entry(path).or_default(), &entries);
            }
        }
    }
    for (path, entries) in &base {
        motion.declare(path, &transition_line(entries));
    }
    for ((to, path), entries) in states {
        let in_base = |entry: &(String, String)| base.get(&path).is_some_and(|b| b.contains(entry));
        if !entries.iter().all(in_base) {
            motion.states.insert((to, path), transition_line(&entries));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Effect {
    Ripple,
    Spinner,
    Pulse,
}

fn effect_of(n: &Node) -> Option<Effect> {
    let flagged = n.plugin_data.get("uigen.effect").map(|v| fold(v));
    match (flagged.as_deref(), fold(&n.name).as_str()) {
        (Some("ripple"), _) | (None, "ripple") => Some(Effect::Ripple),
        (Some("spinner"), _) | (None, "spinner") => Some(Effect::Spinner),
        (Some("pulse"), _) => Some(Effect::Pulse),
        _ => None,
    }
}

fn host_rule(selector: &str, class: &str, decl: &str) -> String {
    format!("{selector} .{class} {{\n  {decl}\n}}\n")
}

/// What the motion of a component is made of: its layers and the interactions between its variants.
struct Scene<'a> {
    doc: &'a Document,
    c: &'a Component,
    tag: String,
    lay: &'a Layout,
    edges: &'a [Edge],
    main: &'a [Rule],
    ids: &'a [(NodeId, String)],
}

impl Scene<'_> {
    /// Rules and keyframes for the effect layers; whether there is any.
    fn effects(&self, motion: &mut Motion) -> bool {
        let layers: Vec<(Effect, &str, &Node)> = self
            .ids
            .iter()
            .filter(|(id, _)| *id != self.c.comp)
            .filter_map(|(id, class)| {
                let n = self.doc.node(*id)?;
                Some((effect_of(n)?, class.as_str(), n))
            })
            .collect();
        for (effect, class, n) in &layers {
            let Some((path, _, _)) = self.main.iter().find(|(_, c, _)| c == class) else { continue };
            match effect {
                Effect::Ripple => self.ripple(motion, class, n, path),
                Effect::Spinner => {
                    motion.tail.push_str(&host_rule(&self.loading(), class, &format!("animation: {}-spin 1s linear infinite;", self.tag)))
                }
                Effect::Pulse => {
                    motion.tail.push_str(&host_rule(&self.loading(), class, &format!("animation: {}-pulse 1.5s ease-in-out infinite;", self.tag)))
                }
            }
        }
        for (effect, keyframes) in self.keyframes() {
            if layers.iter().any(|(e, ..)| *e == effect) {
                motion.tail.push_str(&keyframes);
            }
        }
        !layers.is_empty()
    }

    fn ripple(&self, motion: &mut Motion, class: &str, layer: &Node, path: &str) {
        motion.declare(path, "opacity: 0;\npointer-events: none;");
        motion.declare("", "overflow: hidden;");
        let pressed = self.edges.iter().find(|e| e.entering && e.kind == StateKind::Active).map(|e| e.timing.ms);
        if pressed.is_none() {
            motion.note("ripple-default-timing", Some(layer.name.as_str()), "no interaction presses this component: the ripple lasts 600ms");
        }
        let animation = format!("animation: {}-ripple {}ms ease-out;", self.tag, pressed.unwrap_or(DEFAULT_RIPPLE_MS));
        motion.tail.push_str(&host_rule(":host(:active)", class, &animation));
    }

    /// The host selector of the loading state: its boolean attribute.
    fn loading(&self) -> String {
        let declared = || {
            let params = self.c.params.iter().zip(&self.lay.members);
            params.into_iter().find(|(p, _)| p.kind == ParamKind::Boolean && fold(&p.prop) == "loading").and_then(|(_, m)| m.attribute.clone())
        };
        let from_state = self.lay.state.as_ref().and_then(|s| s.boolean(StateKind::Loading)).map(|(_, _, attribute)| attribute.clone());
        format!(":host([{}])", from_state.or_else(declared).unwrap_or_else(|| "loading".to_string()))
    }

    fn keyframes(&self) -> [(Effect, String); 3] {
        let tag = &self.tag;
        [
            (
                Effect::Ripple,
                format!(
                    "@keyframes {tag}-ripple {{\n  from {{\n    transform: scale(0);\n    opacity: 0.35;\n  }}\n  to {{\n    transform: scale(4);\n    opacity: 0;\n  }}\n}}\n"
                ),
            ),
            (Effect::Spinner, format!("@keyframes {tag}-spin {{\n  to {{\n    transform: rotate(360deg);\n  }}\n}}\n")),
            (Effect::Pulse, format!("@keyframes {tag}-pulse {{\n  50% {{\n    opacity: 0.4;\n  }}\n}}\n")),
        ]
    }
}
