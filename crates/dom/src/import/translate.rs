//! The full translation: the lot 0 layers, cleaned of empty wrappers, named uniquely, with auto
//! layout where the browser's flex layout can be reproduced, and the facts of every layer.

use std::collections::BTreeMap;

use figcraft_color::Color;
use figcraft_doc::proto::Easing;
use figcraft_doc::{Document, NodeId};

use super::{Walk, cleanup, facts, finite_or_zero, layout, motion, names};
use crate::error::ImportError;
use crate::model::Snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslateOptions {
    pub auto_layout: bool,
    pub cleanup: bool,
}

impl Default for TranslateOptions {
    fn default() -> Self {
        Self { auto_layout: true, cleanup: true }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Translated {
    pub root: NodeId,
    pub warnings: Vec<String>,
    pub layers: Vec<LayerFacts>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerFacts {
    pub id: NodeId,
    pub path: String,
    pub tokens: Vec<TokenFact>,
    pub transition: Option<TransitionFact>,
    pub effect: Option<Effect>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TokenFact {
    pub field: String,
    pub value: TokenValue,
    pub exposed: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenValue {
    Color(Color),
    Number(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransitionFact {
    pub properties: Vec<String>,
    pub duration_ms: u32,
    pub delay_ms: u32,
    pub easing: Easing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Ripple,
    Spinner,
}

/// Translate `snapshot` under `parent`, its root frame at `(x, y)`; each warning is also a note on its layer.
pub fn translate_snapshot(
    doc: &mut Document,
    snapshot: &Snapshot,
    parent: NodeId,
    x: f64,
    y: f64,
    options: &TranslateOptions,
) -> Result<Translated, ImportError> {
    if doc.node(parent).is_none() {
        return Err(ImportError::ParentNotFound(parent));
    }
    let mut walk = Walk::new(doc, true);
    let root = walk.run(&snapshot.root, parent, (finite_or_zero(x), finite_or_zero(y)))?;
    let Walk { doc, mut warnings, elements, .. } = walk;
    let sources: BTreeMap<_, _> = elements.into_iter().collect();
    if options.cleanup {
        cleanup::run(doc, root, &sources)?;
    }
    names::make_unique(doc, root);
    if options.auto_layout {
        layout::apply(doc, root, &sources, &mut warnings);
    }
    let mut layers = names::facts(doc, root);
    facts::fill(doc, &sources, &mut layers);
    motion::apply(doc, &sources, &mut warnings, &mut layers);
    Ok(Translated { root, warnings: warnings.into_messages(), layers })
}
