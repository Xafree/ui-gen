//! Which exports of a library are parts of a compound rather than components to offer.

use std::collections::HashSet;

const PART_WORDS: [&str; 22] = [
    "Root", "Body", "Header", "Footer", "Content", "Title", "Heading", "Description", "Trigger", "Item", "Label", "Icon", "Separator", "Backdrop",
    "Container", "Indicator", "Thumb", "Track", "Fill", "Value", "Close", "CloseTrigger",
];

/// `components`: the names offered, each with the export it is a static of, when the service could
/// read it. Otherwise a name is a part when it is an exported name followed by a word for a part.
pub fn parts(components: &[(&str, Option<&str>)]) -> HashSet<String> {
    let names: HashSet<&str> = components.iter().map(|(name, _)| *name).collect();
    let by_prefix = |name: &str| {
        PART_WORDS.iter().any(|word| name.strip_suffix(word).is_some_and(|parent| !parent.is_empty() && names.contains(parent)))
    };
    components
        .iter()
        .filter(|(name, part_of)| part_of.is_some_and(|parent| names.contains(parent)) || by_prefix(name))
        .map(|(name, _)| (*name).to_string())
        .collect()
}
