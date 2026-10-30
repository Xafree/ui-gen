//! Playground targets (E7-2.1): which variables of a theme the Accent, Base, Font and Radius
//! settings drive. The synthetic theme built here is shared with the overrides tests.

use std::collections::{BTreeMap, BTreeSet};

use figcraft_color::Color;
use figcraft_doc::{Collection, Document, Mode, Variable, VariableType, VariableValue};

use crate::playground::targets;

pub(crate) const THEME: &str = "c-theme";
const PRIMITIVES: &str = "c-prim";
pub(crate) const LIGHT: &str = "m-light";
pub(crate) const DARK: &str = "m-dark";
const PRIMITIVE_MODE: &str = "m-prim";

pub(crate) struct Fixture {
    pub doc: Document,
    pub theme: Collection,
}

impl Fixture {
    pub fn id(&self, name: &str) -> String {
        self.var(name).id.clone()
    }

    pub fn var(&self, name: &str) -> &Variable {
        self.doc.variables.variables.iter().find(|v| v.name == name).unwrap_or_else(|| panic!("no variable {name} in the fixture"))
    }
}

pub(crate) fn color(hex: &str) -> Color {
    Color::from_hex(hex).unwrap_or_else(|| panic!("not a colour: {hex}"))
}

fn variable(name: &str, collection: &str, kind: VariableType, values: Vec<(&str, VariableValue)>, scopes: Option<&str>) -> Variable {
    Variable {
        id: format!("v:{name}"),
        name: name.to_string(),
        collection: collection.to_string(),
        kind,
        values: values.into_iter().map(|(mode, value)| (mode.to_string(), value)).collect(),
        description: String::new(),
        scopes: scopes.map(|s| vec![s.to_string()]),
        hidden_from_publishing: false,
        extends: None,
        overrides: BTreeSet::new(),
        code_syntax: BTreeMap::new(),
    }
}

pub(crate) fn themed_color(name: &str, light: Color, dark: Color) -> Variable {
    variable(name, THEME, VariableType::Color, vec![(LIGHT, VariableValue::Color(light)), (DARK, VariableValue::Color(dark))], None)
}

pub(crate) fn themed_number(name: &str, light: f64, dark: f64, scope: &str) -> Variable {
    variable(name, THEME, VariableType::Number, vec![(LIGHT, VariableValue::Number(light)), (DARK, VariableValue::Number(dark))], Some(scope))
}

pub(crate) fn themed_string(name: &str, value: &str, scope: Option<&str>) -> Variable {
    let text = || VariableValue::String(value.to_string());
    variable(name, THEME, VariableType::String, vec![(LIGHT, text()), (DARK, text())], scope)
}

fn aliased(name: &str, target: &str) -> Variable {
    let alias = || VariableValue::Alias(format!("v:{target}"));
    variable(name, THEME, VariableType::Color, vec![(LIGHT, alias()), (DARK, alias())], None)
}

fn spec_variables() -> Vec<Variable> {
    vec![
        themed_color("couleur/primaire", color("#E5532D"), color("#FF7A59")),
        themed_color("couleur/primaire/texte", color("#FFFFFF"), color("#FFFFFF")),
        themed_color("couleur/primaire-contraste", color("#0B3D91"), color("#9DB9F5")),
        themed_color("couleur/fond", color("#F5F5F5"), color("#1A1A1A")),
        themed_color("couleur/bordure", color("#D8D3CE"), color("#333333")),
        aliased("couleur/carte", "gris/500"),
        themed_color("couleur/blanc", color("#FFFFFF"), color("#FFFFFF")),
        themed_color("couleur/noir", color("#000000"), color("#000000")),
        themed_color("couleur/succes", color("#22AA55"), color("#33CC77")),
        themed_string("typo/police-famille", "Inter", None),
        themed_number("rayon/bouton", 8.0, 10.0, "CORNER_RADIUS"),
        themed_number("espace/gouttiere", 16.0, 16.0, "GAP"),
    ]
}

pub(crate) fn fixture_from(variables: Vec<Variable>) -> Fixture {
    let mut doc = Document::new("playground fixture");
    let primitives = Collection {
        id: PRIMITIVES.to_string(),
        name: "Primitives".to_string(),
        modes: vec![Mode { id: PRIMITIVE_MODE.to_string(), name: "Value".to_string() }],
        extends: None,
    };
    let theme = Collection {
        id: THEME.to_string(),
        name: "Theme".to_string(),
        modes: vec![Mode { id: LIGHT.to_string(), name: "Light".to_string() }, Mode { id: DARK.to_string(), name: "Dark".to_string() }],
        extends: None,
    };
    doc.variables.collections.push(primitives);
    doc.variables.collections.push(theme.clone());
    let grey = variable("gris/500", PRIMITIVES, VariableType::Color, vec![(PRIMITIVE_MODE, VariableValue::Color(color("#808080")))], None);
    doc.variables.variables.push(grey);
    doc.variables.variables.extend(variables);
    Fixture { doc, theme }
}

pub(crate) fn fixture() -> Fixture {
    fixture_from(spec_variables())
}

pub(crate) fn fixture_with(extra: Vec<Variable>) -> Fixture {
    let mut all = spec_variables();
    all.extend(extra);
    fixture_from(all)
}

pub(crate) fn fixture_without(dropped: &[&str]) -> Fixture {
    fixture_from(spec_variables().into_iter().filter(|v| !dropped.contains(&v.name.as_str())).collect())
}

fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

fn ids(f: &Fixture, names: &[&str]) -> Vec<String> {
    sorted(names.iter().map(|n| f.id(n)).collect())
}

// covers: E7-2.1
#[test]
fn targets_of_a_synthetic_theme_follow_the_detection_rules() {
    let f = fixture();
    let found = targets(&f.doc, &f.theme.id);
    assert_eq!(sorted(found.accent), ids(&f, &["couleur/primaire"]));
    assert_eq!(sorted(found.base), ids(&f, &["couleur/fond", "couleur/bordure", "couleur/carte"]));
    assert_eq!(sorted(found.font), ids(&f, &["typo/police-famille"]));
    assert_eq!(sorted(found.radius), ids(&f, &["rayon/bouton"]));
}

// covers: E7-2.1
#[test]
fn a_theme_without_an_accent_token_has_no_accent_target() {
    let f = fixture_without(&["couleur/primaire", "couleur/primaire/texte", "couleur/primaire-contraste"]);
    let found = targets(&f.doc, &f.theme.id);
    assert!(found.accent.is_empty(), "accent targets: {:?}", found.accent);
    assert_eq!(sorted(found.base), ids(&f, &["couleur/fond", "couleur/bordure", "couleur/carte"]));
}

// covers: E7-2.1
#[test]
fn accent_words_are_read_in_camel_case_and_the_on_word_excludes() {
    let f = fixture_with(vec![
        themed_color("couleurPrimaireSurvol", color("#E5532D"), color("#FF7A59")),
        themed_color("couleur/on-primaire", color("#0B3D91"), color("#9DB9F5")),
        themed_color("couleur/accent", color("#E5532D"), color("#FF7A59")),
    ]);
    let found = targets(&f.doc, &f.theme.id);
    assert_eq!(sorted(found.accent), ids(&f, &["couleur/primaire", "couleurPrimaireSurvol", "couleur/accent"]));
}

// covers: E7-2.1
#[test]
fn base_reads_the_light_mode_value_and_font_is_found_by_scope() {
    let f = fixture_with(vec![
        themed_color("couleur/vif-clair", color("#FF3300"), color("#808080")),
        themed_color("couleur/neutre-clair", color("#808080"), color("#FF3300")),
        themed_string("corps", "Georgia", Some("FONT_FAMILY")),
        themed_string("titre", "Bonjour", Some("TEXT_CONTENT")),
    ]);
    let found = targets(&f.doc, &f.theme.id);
    let base = sorted(found.base);
    assert!(base.contains(&f.id("couleur/neutre-clair")), "base targets: {base:?}");
    assert!(!base.contains(&f.id("couleur/vif-clair")), "base targets: {base:?}");
    assert_eq!(sorted(found.font), ids(&f, &["typo/police-famille", "corps"]));
}

// covers: E7-2.1
#[test]
fn only_radius_role_numbers_are_radius_targets() {
    let f = fixture_with(vec![themed_number("rayon/carte", 6.0, 6.0, "CORNER_RADIUS"), themed_number("opacite/voile", 0.5, 0.5, "OPACITY")]);
    let found = targets(&f.doc, &f.theme.id);
    assert_eq!(sorted(found.radius), ids(&f, &["rayon/bouton", "rayon/carte"]));
}
