use std::fmt;

use figcraft_color::{Color, Oklch};
use figcraft_doc::{Collection, Document, ModeId, Variable, VariableId, VariableValue};

use super::settings::{Mode, Neutral, Settings};
use super::targets::{Targets, color_in, targets, value_in};
use crate::codegen::LitNames;
use crate::dev::lit_export::{dark_of, light_of};
use crate::token_roles::Roles;

const MAX_FONT_CHARS: usize = 64;
const MAX_TINT_CHROMA: f64 = 0.04;
const FORBIDDEN_IN_FONT: [char; 7] = [';', '{', '}', '(', ')', '\\', '"'];
const GENERIC_FAMILIES: [&str; 10] =
    ["system-ui", "serif", "sans-serif", "monospace", "cursive", "fantasy", "ui-serif", "ui-sans-serif", "ui-monospace", "ui-rounded"];
const LIGHT_SELECTOR: &str = ":root[data-theme=\"light\"]";
const DARK_SELECTOR: &str = ":root[data-theme=\"dark\"]";

/// The custom properties the settings rewrite: `(--name, value)` in the order of the variables,
/// for each mode.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    pub light: Vec<(String, String)>,
    pub dark: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingError {
    InvalidFont(String),
    InvalidHue,
    InvalidTint,
    NoTarget(&'static str),
}

impl fmt::Display for SettingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingError::InvalidFont(font) => {
                write!(f, "font: {font:?} is not a font name (1 to {MAX_FONT_CHARS} characters, none of ; {{ }} ( ) \\ \" or a line break)")
            }
            SettingError::InvalidHue => write!(f, "base: the hue is a number from 0 to 360"),
            SettingError::InvalidTint => write!(f, "base: the tint is a number from 0 to 1"),
            SettingError::NoTarget(setting) => write!(f, "{setting}: no token of this theme answers to this setting"),
        }
    }
}

impl std::error::Error for SettingError {}

/// What the settings show, the theme's own values where nothing is set.
#[derive(Clone, Debug, PartialEq)]
pub struct Effective {
    pub accent: Option<Color>,
    pub base: Option<Neutral>,
    pub font: Option<String>,
    pub radius: Option<f64>,
}

fn validate(settings: &Settings) -> Result<(), SettingError> {
    if let Some(base) = settings.base {
        if !(0.0..=360.0).contains(&base.hue) {
            return Err(SettingError::InvalidHue);
        }
        if !(0.0..=1.0).contains(&base.tint) {
            return Err(SettingError::InvalidTint);
        }
    }
    match &settings.font {
        Some(font) if !is_font_name(font) => Err(SettingError::InvalidFont(font.clone())),
        _ => Ok(()),
    }
}

pub(crate) fn is_font_name(font: &str) -> bool {
    !font.trim().is_empty() && font.chars().count() <= MAX_FONT_CHARS && !font.chars().any(|c| c.is_control() || FORBIDDEN_IN_FONT.contains(&c))
}

fn require_targets(settings: &Settings, found: &Targets) -> Result<(), SettingError> {
    let missing = [
        ("accent", settings.accent.is_some() && found.accent.is_empty()),
        ("base", settings.base.is_some() && found.base.is_empty()),
        ("font", settings.font.is_some() && found.font.is_empty()),
        ("radius", settings.radius.is_some() && found.radius.is_empty()),
    ];
    match missing.into_iter().find(|(_, missing)| *missing) {
        Some((setting, _)) => Err(SettingError::NoTarget(setting)),
        None => Ok(()),
    }
}

fn mode_id(collection: &Collection, mode: Mode) -> Option<String> {
    let light = light_of(collection, None)?;
    let chosen = match mode {
        Mode::Light => light,
        Mode::Dark => dark_of(collection, None, light).unwrap_or(light),
    };
    Some(chosen.id.clone())
}

struct Rewriter<'a> {
    doc: &'a Document,
    collection: &'a Collection,
    settings: &'a Settings,
    found: Targets,
    roles: Roles,
}

/// One value the settings rewrite: `variable` in mode `mode` of its collection.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Rewrite {
    pub variable: VariableId,
    pub mode: ModeId,
    pub value: VariableValue,
}

impl Rewriter<'_> {
    fn value(&self, v: &Variable, mode: &str) -> Option<VariableValue> {
        let (settings, found) = (self.settings, &self.found);
        if let Some(accent) = settings.accent.filter(|_| found.accent.contains(&v.id)) {
            let chosen = Oklch::from_color(accent);
            return self.recolored(v, mode, chosen.c, chosen.h);
        }
        if let Some(base) = settings.base.filter(|_| found.base.contains(&v.id)) {
            return self.recolored(v, mode, (base.tint * MAX_TINT_CHROMA) as f32, base.hue as f32);
        }
        if let Some(font) = settings.font.as_ref().filter(|_| found.font.contains(&v.id)) {
            return Some(VariableValue::String(font.clone()));
        }
        let preset = settings.radius.filter(|_| found.radius.contains(&v.id))?;
        let VariableValue::Number(x) = value_in(self.doc, self.collection, v, mode)? else { return None };
        preset.scale(x).filter(|s| s.is_finite()).map(VariableValue::Number)
    }

    fn recolored(&self, v: &Variable, mode: &str, chroma: f32, hue: f32) -> Option<VariableValue> {
        let original = color_in(self.doc, self.collection, v, mode)?;
        let lightness = Oklch::from_color(original).l;
        Some(VariableValue::Color(Oklch { l: lightness, c: chroma, h: hue }.to_color().with_alpha(original.a)))
    }

    fn css(&self, v: &Variable, value: &VariableValue) -> Option<String> {
        match value {
            VariableValue::Color(c) => Some(c.to_css()),
            VariableValue::String(font) => Some(font_family(font)),
            VariableValue::Number(x) => Some(self.roles.number_css(v, *x)),
            VariableValue::Boolean(_) | VariableValue::Alias(_) => None,
        }
    }
}

fn font_family(font: &str) -> String {
    let name = font.trim();
    if GENERIC_FAMILIES.iter().any(|generic| generic.eq_ignore_ascii_case(name)) { name.to_string() } else { format!("\"{font}\"") }
}

fn rewriter<'a>(doc: &'a Document, collection: &'a Collection, settings: &'a Settings) -> Result<Rewriter<'a>, SettingError> {
    validate(settings)?;
    let found = targets(doc, &collection.id);
    require_targets(settings, &found)?;
    Ok(Rewriter { doc, collection, settings, found, roles: Roles::new(doc) })
}

/// The custom properties `settings` rewrite in `collection`, each mode starting from its own
/// values. A collection with no dark mode gives the light values to the dark page.
pub(crate) fn overrides(doc: &Document, collection: &Collection, names: &LitNames, settings: &Settings) -> Result<Overrides, SettingError> {
    let rewriter = rewriter(doc, collection, settings)?;
    let mut out = Overrides::default();
    let (Some(light), Some(dark)) = (mode_id(collection, Mode::Light), mode_id(collection, Mode::Dark)) else { return Ok(out) };
    for v in doc.variables.variables.iter().filter(|v| v.collection == collection.id) {
        for (mode, list) in [(&light, &mut out.light), (&dark, &mut out.dark)] {
            if let Some(css) = rewriter.value(v, mode).and_then(|value| rewriter.css(v, &value)) {
                list.push((names.name(v), css));
            }
        }
    }
    Ok(out)
}

/// The values `settings` rewrite in every mode of `collection`, for saving them into a theme.
pub(crate) fn rewrites(doc: &Document, collection: &Collection, settings: &Settings) -> Result<Vec<Rewrite>, SettingError> {
    let rewriter = rewriter(doc, collection, settings)?;
    let mut out = Vec::new();
    for v in doc.variables.variables.iter().filter(|v| v.collection == collection.id) {
        for mode in &collection.modes {
            if let Some(value) = rewriter.value(v, &mode.id) {
                out.push(Rewrite { variable: v.id.clone(), mode: mode.id.clone(), value });
            }
        }
    }
    Ok(out)
}

fn block(out: &mut String, selector: &str, list: &[(String, String)]) {
    if list.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&format!("{selector} {{\n"));
    for (name, value) in list {
        out.push_str(&format!("  {name}: {value};\n"));
    }
    out.push_str("}\n");
}

/// The overrides as a style sheet, the light block then the dark one; empty when nothing is set.
pub fn overrides_css(overrides: &Overrides) -> String {
    let mut out = String::new();
    block(&mut out, LIGHT_SELECTOR, &overrides.light);
    block(&mut out, DARK_SELECTOR, &overrides.dark);
    out
}

pub fn effective(doc: &Document, collection: &Collection, settings: &Settings) -> Effective {
    let found = targets(doc, &collection.id);
    let mode = mode_id(collection, settings.mode);
    let first = |ids: &[String]| ids.first().and_then(|id| doc.variables.get(id));
    let value_of = |v: Option<&Variable>| match (v, mode.as_deref()) {
        (Some(v), Some(mode)) => value_in(doc, collection, v, mode),
        _ => None,
    };
    let accent = settings.accent.or_else(|| match value_of(first(&found.accent)) {
        Some(VariableValue::Color(c)) => Some(c),
        _ => None,
    });
    let base = settings.base.or_else(|| match value_of(first(&found.base)) {
        Some(VariableValue::Color(c)) => {
            let k = Oklch::from_color(c);
            Some(Neutral { hue: f64::from(k.h), tint: (f64::from(k.c) / MAX_TINT_CHROMA).clamp(0.0, 1.0) })
        }
        _ => None,
    });
    let font = settings.font.clone().or_else(|| match value_of(first(&found.font)) {
        Some(VariableValue::String(s)) => Some(s),
        _ => None,
    });
    let radius = match value_of(first(&found.radius)) {
        Some(VariableValue::Number(x)) if x.is_finite() => Some(settings.radius.and_then(|p| p.scale(x)).unwrap_or(x)),
        _ => None,
    };
    Effective { accent, base, font, radius }
}
