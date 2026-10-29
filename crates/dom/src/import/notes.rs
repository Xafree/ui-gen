//! What a layer could not carry is written on it as a Dev Mode annotation, next to the warning.

use figcraft_doc::{Annotation, Document, NodeId};

pub(super) const NOTE_PREFIX: &str = "UIGEN import: ";

pub(super) fn annotations(messages: Vec<String>) -> Vec<Annotation> {
    messages.into_iter().map(|message| Annotation { label: format!("{NOTE_PREFIX}{message}"), ..Annotation::default() }).collect()
}

pub(super) fn attach(doc: &mut Document, id: NodeId, messages: Vec<String>) {
    if let Some(node) = doc.node_mut(id) {
        node.annotations.extend(annotations(messages));
    }
}
