use std::collections::BTreeMap;

use figcraft_color::{Color, Oklch};
use figcraft_doc::{Collection, CollectionId, Document, Variable, VariableId, VariableType, VariableValue};

use crate::dev::lit_export::light_of;
use crate::token_roles::{NumberRole, Roles};

const NEUTRAL_MAX_CHROMA: f32 = 0.04;
const NEUTRAL_LIGHTNESS: std::ops::RangeInclusive<f32> = 0.1..=0.985;
const ACCENT_WORDS: [&str; 3] = ["primary", "primaire", "accent"];
const NOT_ACCENT_WORDS: [&str; 6] = ["contrast", "contraste", "text", "texte", "foreground", "on"];
const FONT_NAMES: [&str; 3] = ["font", "police", "typo"];
const FAMILY_NAMES: [&str; 2] = ["family", "famille"];

/// The variables of a theme each setting drives, in the order of the file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Targets {
    pub accent: Vec<VariableId>,
    pub base: Vec<VariableId>,
    pub font: Vec<VariableId>,
    pub radius: Vec<VariableId>,
}

pub fn targets(doc: &Document, collection: &CollectionId) -> Targets {
    let Some(coll) = doc.variables.collection(collection) else { return Targets::default() };
    let roles = Roles::new(doc);
    let light = light_of(coll, None);
    let mut found = Targets::default();
    for v in doc.variables.variables.iter().filter(|v| v.collection == coll.id) {
        match v.kind {
            VariableType::Color if is_accent(&v.name) => found.accent.push(v.id.clone()),
            VariableType::Color if light.is_some_and(|m| is_neutral(color_in(doc, coll, v, &m.id))) => found.base.push(v.id.clone()),
            VariableType::String if is_font(v) => found.font.push(v.id.clone()),
            VariableType::Number if roles.role(v) == NumberRole::Radius => found.radius.push(v.id.clone()),
            _ => {}
        }
    }
    found
}

/// `variable` in mode `mode` of `collection`, aliases followed (an alias into the same collection
/// reads the same mode).
pub(super) fn value_in(doc: &Document, collection: &Collection, variable: &Variable, mode: &str) -> Option<VariableValue> {
    let modes: BTreeMap<CollectionId, String> = BTreeMap::from([(collection.id.clone(), mode.to_string())]);
    doc.variables.resolve(&variable.id, &modes)
}

pub(super) fn color_in(doc: &Document, collection: &Collection, variable: &Variable, mode: &str) -> Option<Color> {
    match value_in(doc, collection, variable, mode)? {
        VariableValue::Color(c) => Some(c),
        _ => None,
    }
}

fn is_neutral(color: Option<Color>) -> bool {
    color.map(Oklch::from_color).is_some_and(|k| k.c <= NEUTRAL_MAX_CHROMA && NEUTRAL_LIGHTNESS.contains(&k.l))
}

/// The lower-case words of a name, cut on `/ - _ .`, spaces and capitals.
fn words(name: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut word = String::new();
    let mut previous: Option<char> = None;
    for c in name.chars() {
        let separator = matches!(c, '/' | '-' | '_' | '.') || c.is_whitespace();
        let camel = c.is_uppercase() && previous.is_some_and(|p| p.is_lowercase() || p.is_ascii_digit());
        if (separator || camel) && !word.is_empty() {
            found.push(std::mem::take(&mut word));
        }
        if !separator {
            word.extend(c.to_lowercase());
        }
        previous = Some(c);
    }
    if !word.is_empty() {
        found.push(word);
    }
    found
}

fn is_accent(name: &str) -> bool {
    let words = words(name);
    let has = |list: &[&str]| words.iter().any(|w| list.contains(&w.as_str()));
    has(&ACCENT_WORDS) && !has(&NOT_ACCENT_WORDS)
}

fn is_font(v: &Variable) -> bool {
    if v.scopes.as_ref().is_some_and(|s| s.iter().any(|s| s == "FONT_FAMILY")) {
        return true;
    }
    let name = v.name.to_lowercase();
    FONT_NAMES.iter().any(|f| name.contains(f)) && FAMILY_NAMES.iter().any(|f| name.contains(f))
}
