//! The state transitions of an imported set: from each `Default` variant, hovering and pressing
//! change to the `Hover` and `Pressed` variants of the same style and size, timed like the transition
//! the arrival variant declared. What the prototype cannot say becomes a note on the layer.

use figcraft_doc::proto::{Action, Easing, Interaction, Transition, Trigger};
use figcraft_doc::{Annotation, Document, NodeId};
use figcraft_dom::{LayerFacts, NOTE_PREFIX, TransitionFact};

use super::plan::State;

use crate::Result;

pub(super) struct Variant<'a> {
    pub id: NodeId,
    pub style: Option<&'a str>,
    pub size: Option<&'a str>,
    pub state: State,
    pub layers: &'a [LayerFacts],
}

const GESTURES: [(State, fn() -> Trigger); 2] = [(State::Hover, || Trigger::OnHover), (State::Pressed, || Trigger::OnPress)];

pub(super) fn apply(doc: &mut Document, variants: &[Variant]) -> Result<()> {
    let mut notes: Vec<(NodeId, String)> = Vec::new();
    let mut interactions: Vec<(NodeId, Interaction)> = Vec::new();
    for rest in variants.iter().filter(|variant| variant.state == State::Default) {
        for (state, trigger) in GESTURES {
            let Some(arrival) =
                variants.iter().find(|candidate| candidate.state == state && candidate.style == rest.style && candidate.size == rest.size)
            else {
                continue;
            };
            interactions.push((rest.id, interaction(arrival, trigger(), &mut notes)));
        }
    }
    for focus in variants.iter().filter(|variant| variant.state == State::Focus) {
        for layer in focus.layers {
            if let Some(fact) = &layer.transition {
                notes.push((layer.id, format!("{} left out: keyboard focus has no prototype trigger", described(fact))));
            }
        }
    }
    for (id, interaction) in interactions {
        doc.get_mut(id)?.interactions.push(interaction);
    }
    for (id, message) in notes {
        let label = format!("{NOTE_PREFIX}transition {message}");
        let node = doc.get_mut(id)?;
        if !node.annotations.iter().any(|note| note.label == label) {
            node.annotations.push(Annotation { label, ..Annotation::default() });
        }
    }
    Ok(())
}

fn interaction(arrival: &Variant, trigger: Trigger, notes: &mut Vec<(NodeId, String)>) -> Interaction {
    let longest = longest(arrival.layers);
    let timing = longest.map(|(layer, fact)| {
        if fact.delay_ms > 0 {
            notes.push((layer.id, format!("{} delay left out: a prototype interaction has no delay on hover or press", described(fact))));
        }
        for other in arrival.layers {
            if let Some(shorter) = other.transition.as_ref().filter(|candidate| !same_timing(candidate, fact)) {
                notes.push((other.id, format!("{} left out: only the longest transition ({}ms) is kept", described(shorter), fact.duration_ms)));
            }
        }
        fact
    });
    Interaction {
        trigger,
        actions: vec![Action::ChangeTo { dest: arrival.id }],
        transition: timing.map_or(Transition::Instant, |_| Transition::SmartAnimate),
        duration_ms: timing.map_or(0, |fact| fact.duration_ms),
        easing: timing.map_or_else(Easing::default, |fact| fact.easing),
        delay_ms: 0,
    }
}

fn longest(layers: &[LayerFacts]) -> Option<(&LayerFacts, &TransitionFact)> {
    layers
        .iter()
        .filter_map(|layer| layer.transition.as_ref().map(|fact| (layer, fact)))
        .reduce(|best, candidate| if candidate.1.duration_ms > best.1.duration_ms { candidate } else { best })
}

fn same_timing(a: &TransitionFact, b: &TransitionFact) -> bool {
    a.duration_ms == b.duration_ms && a.delay_ms == b.delay_ms && a.easing == b.easing
}

fn described(fact: &TransitionFact) -> String {
    format!("{} {}ms", fact.properties.join(", "), fact.duration_ms)
}
