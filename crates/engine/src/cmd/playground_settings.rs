//! The playground settings commands: `set`, `undo`, `redo`, `revert` and `random` change the settings
//! the playground keeps apart from the document and push the token overrides to the page; `saveTheme`
//! writes them into a collection of the document as one undo step; `code` shows what the page runs.

use figcraft_color::Color;
use figcraft_doc::Document;
use serde_json::{Map, Value, json};

use super::CommandSpec;
use super::playground::{NOT_OPEN, ensure_ready, shown};
use crate::params::get;
use crate::playground::{
    Effective, History, Mode, Neutral, PlaygroundState, RadiusPreset, RadiusShape, Settings, copy_with, effective, is_font_name, random_settings,
    rewrites, targets, write_into,
};
use crate::{Result, Session, bad, spec};

const SET: &str = "playground.set";
const UNDO: &str = "playground.undo";
const REDO: &str = "playground.redo";
const REVERT: &str = "playground.revert";
const RANDOM: &str = "playground.random";
pub(super) const SAVE: &str = "playground.saveTheme";
const CODE: &str = "playground.code";
const MAX_NAME_CHARS: usize = 64;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            SET,
            "Set playground settings",
            "{accent? (#rrggbb | null), base? ({hue (0 to 360), tint (0 to 1)} | null), font? (a font name | null), radius? (zero|small|medium|large|full | null), radiusShape? (round|squircle|bevel), theme? (a collection name | null), mode? (light|dark), commit? (default true; false shows the change without making a history step)} → {settings, effective, targets, canUndo, canRedo}  changes the settings of the open playground and pushes the token overrides to the page; the document does not change. Choosing another theme clears the other settings. A setting no token of the theme answers to is refused",
            false,
            set
        ),
        spec!(
            UNDO,
            "Undo playground setting",
            "{} → {settings, effective, targets, canUndo, canRedo}  the previous settings of the playground, apart from edit.undo",
            false,
            undo
        ),
        spec!(
            REDO,
            "Redo playground setting",
            "{} → {settings, effective, targets, canUndo, canRedo}  the settings undone, apart from edit.redo",
            false,
            redo
        ),
        spec!(
            REVERT,
            "Revert playground to the applied theme",
            "{} → {settings, effective, targets, canUndo, canRedo}  clears every setting, as one step the playground can undo",
            false,
            revert
        ),
        spec!(
            RANDOM,
            "Randomise playground settings",
            "{seed? (a whole number; default the clock)} → {settings, effective, targets, canUndo, canRedo}  draws an accent, a base, a font and a radius; the same seed gives the same settings; the mode, the theme and the radius shape stay",
            false,
            random
        ),
        spec!(
            SAVE,
            "Save playground theme",
            "{name, update? (false: a new collection named `name`, a copy of the chosen theme with the settings written in every mode; true: rewrite the chosen theme, `name` is its name), apply? (make the result the applied theme)} → {collection, name}  one undo step. The radius shape is not saved",
            true,
            save_theme
        ),
        spec!(
            CODE,
            "Playground code",
            "{} → {overrides (the style sheet pushed to the page), template (the HTML of the model), notes: [text]}  nothing is written",
            false,
            code
        ),
    ]
}

fn field<'a>(p: &'a Value, key: &str) -> Option<&'a Value> {
    p.get(key)
}

fn flag(p: &Value, cmd: &str, key: &str, default: bool) -> Result<bool> {
    match get(p, key) {
        None => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(bad(cmd, format!("`{key}` must be true or false"))),
    }
}

fn text_of<'a>(cmd: &str, key: &str, value: &'a Value) -> Result<&'a str> {
    value.as_str().ok_or_else(|| bad(cmd, format!("`{key}` must be a string")))
}

fn accent_of(cmd: &str, value: &Value) -> Result<Color> {
    let text = text_of(cmd, "accent", value)?;
    let hex = text.len() == 7 && text.starts_with('#');
    hex.then(|| Color::from_hex(text)).flatten().ok_or_else(|| bad(cmd, "`accent` must be a colour written #rrggbb"))
}

fn base_of(cmd: &str, value: &Value) -> Result<Neutral> {
    let number = |key: &str| value.get(key).and_then(Value::as_f64).ok_or_else(|| bad(cmd, format!("`base.{key}` must be a number")));
    Ok(Neutral { hue: number("hue")?, tint: number("tint")? })
}

fn font_of(cmd: &str, value: &Value) -> Result<String> {
    let text = text_of(cmd, "font", value)?;
    if is_font_name(text) {
        return Ok(text.to_string());
    }
    Err(bad(cmd, format!("font: \"{}\" is not a font name (1 to 64 characters, none of ; {{ }} ( ) \\ \" or a line break)", shown(text))))
}

fn choice<T>(cmd: &str, key: &str, value: &Value, parse: fn(&str) -> Option<T>, allowed: &str) -> Result<T> {
    parse(text_of(cmd, key, value)?).ok_or_else(|| bad(cmd, format!("`{key}` must be one of {allowed}")))
}

fn cleared(settings: &Settings) -> Settings {
    Settings { accent: None, base: None, font: None, radius: None, ..settings.clone() }
}

fn merged(state: &PlaygroundState, cmd: &str, p: &Value) -> Result<Settings> {
    if !p.is_object() {
        return Err(bad(cmd, "params must be an object"));
    }
    let mut settings = state.history().current().clone();
    if let Some(value) = field(p, "theme") {
        let theme = match value {
            Value::Null => None,
            other => Some(text_of(cmd, "theme", other)?.to_string()),
        };
        let before = state.theme_of(&settings).map(|c| c.name.clone());
        settings.theme = theme;
        let after = state.theme_of(&settings).map(|c| c.name.clone());
        if after.is_none() {
            let name = settings.theme.as_deref().unwrap_or_default();
            return Err(bad(cmd, format!("unknown theme \"{}\": no collection of that name in the document", shown(name))));
        }
        if before != after {
            settings = cleared(&settings);
        }
    }
    if let Some(value) = field(p, "accent") {
        settings.accent = if value.is_null() { None } else { Some(accent_of(cmd, value)?) };
    }
    if let Some(value) = field(p, "base") {
        settings.base = if value.is_null() { None } else { Some(base_of(cmd, value)?) };
    }
    if let Some(value) = field(p, "font") {
        settings.font = if value.is_null() { None } else { Some(font_of(cmd, value)?) };
    }
    if let Some(value) = field(p, "radius") {
        let allowed = "zero, small, medium, large, full";
        settings.radius = if value.is_null() { None } else { Some(choice(cmd, "radius", value, RadiusPreset::parse, allowed)?) };
    }
    if let Some(value) = get(p, "radiusShape") {
        settings.radius_shape = choice(cmd, "radiusShape", value, RadiusShape::parse, "round, squircle, bevel")?;
    }
    if let Some(value) = get(p, "mode") {
        settings.mode = choice(cmd, "mode", value, Mode::parse, "light, dark")?;
    }
    Ok(settings)
}

fn hex_of(color: Option<Color>) -> Value {
    color.map_or(Value::Null, |c| json!(c.to_css_hex()))
}

fn base_json(base: Option<Neutral>) -> Value {
    base.map_or(Value::Null, |b| json!({"hue": b.hue, "tint": b.tint}))
}

fn settings_json(settings: &Settings) -> Value {
    json!({
        "accent": hex_of(settings.accent),
        "base": base_json(settings.base),
        "font": settings.font,
        "radius": settings.radius.map(RadiusPreset::id),
        "radiusShape": settings.radius_shape.id(),
        "theme": settings.theme,
        "mode": settings.mode.id(),
    })
}

fn effective_json(shown: &Effective) -> Value {
    json!({"accent": hex_of(shown.accent), "base": base_json(shown.base), "font": shown.font, "radius": shown.radius})
}

fn view(state: &PlaygroundState) -> Map<String, Value> {
    let mut out = Map::new();
    let (Some(target), settings) = (state.target(), state.history().current()) else { return out };
    let Some(theme) = state.theme_of(settings) else { return out };
    let found = targets(&target.doc, &theme.id);
    out.insert("settings".into(), settings_json(settings));
    out.insert("effective".into(), effective_json(&effective(&target.doc, theme, settings)));
    out.insert(
        "targets".into(),
        json!({"accent": found.accent.len(), "base": found.base.len(), "font": found.font.len(), "radius": found.radius.len()}),
    );
    out.insert("canUndo".into(), json!(state.history().can_undo()));
    out.insert("canRedo".into(), json!(state.history().can_redo()));
    out
}

fn applied_of(s: &Session, uid: u64) -> Option<String> {
    let ds = s.docs.iter().find(|ds| ds.uid == uid)?;
    let named = ds.project.as_ref().and_then(|binding| binding.config.applied_theme.clone());
    named.or_else(|| ds.doc.variables.collections.first().map(|c| c.name.clone()))
}

/// What `playground.state` adds to its answer: the settings, the themes to choose from and the
/// components the library lacks; nulls and empty lists while no playground is open.
pub(super) fn summary(s: &Session) -> Value {
    let state = &s.capture.playground;
    let mut out = view(state);
    let target = state.target();
    let themes: Vec<&str> = target.map(|t| t.doc.variables.collections.iter().map(|c| c.name.as_str()).collect()).unwrap_or_default();
    out.insert("themes".into(), json!(themes));
    out.insert("applied".into(), json!(target.and_then(|t| applied_of(s, t.uid))));
    out.insert("missing".into(), json!(target.map(|t| t.composed.missing.clone()).unwrap_or_default()));
    out.insert("notes".into(), json!(target.map(|t| t.plan.notes.clone()).unwrap_or_default()));
    for key in ["settings", "effective", "targets"] {
        out.entry(key).or_insert(Value::Null);
    }
    out.entry("canUndo").or_insert(json!(false));
    out.entry("canRedo").or_insert(json!(false));
    Value::Object(out)
}

fn change(s: &mut Session, cmd: &str, next: History) -> Result<Value> {
    let prepared = s.capture.playground.prepare(cmd, next.current())?;
    s.playground_push(prepared, next).map_err(|e| bad(cmd, e.message))?;
    Ok(Value::Object(view(&s.capture.playground)))
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    ensure_ready(s, SET)?;
    let commit = flag(p, SET, "commit", true)?;
    let state = &s.capture.playground;
    let settings = merged(state, SET, p)?;
    let mut next = state.history().clone();
    if commit {
        next.commit(settings);
    } else {
        next.preview(settings);
    }
    change(s, SET, next)
}

fn walk(s: &mut Session, cmd: &str, step: fn(&mut History) -> bool) -> Result<Value> {
    ensure_ready(s, cmd)?;
    let mut next = s.capture.playground.history().clone();
    if !step(&mut next) {
        return Ok(Value::Object(view(&s.capture.playground)));
    }
    change(s, cmd, next)
}

fn undo(s: &mut Session, _: &Value) -> Result<Value> {
    walk(s, UNDO, History::undo)
}

fn redo(s: &mut Session, _: &Value) -> Result<Value> {
    walk(s, REDO, History::redo)
}

fn revert(s: &mut Session, _: &Value) -> Result<Value> {
    walk(s, REVERT, History::revert)
}

fn seed_of(p: &Value) -> Result<u64> {
    match get(p, "seed") {
        None => {
            Ok(web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(0)))
        }
        Some(value) => value.as_u64().ok_or_else(|| bad(RANDOM, "`seed` must be a whole number, zero or more")),
    }
}

fn installed_fonts() -> Vec<String> {
    let mut fonts: Vec<String> = figcraft_text::family_names().into_iter().filter(|name| is_font_name(name)).collect();
    fonts.sort();
    fonts.dedup();
    fonts
}

fn random(s: &mut Session, p: &Value) -> Result<Value> {
    ensure_ready(s, RANDOM)?;
    let seed = seed_of(p)?;
    let state = &s.capture.playground;
    let current = state.history().current();
    let (Some(target), Some(theme)) = (state.target(), state.theme_of(current)) else { return Err(bad(RANDOM, NOT_OPEN)) };
    let names: Vec<String> = target.doc.variables.collections.iter().map(|c| c.name.clone()).collect();
    let found = targets(&target.doc, &theme.id);
    let mut drawn = random_settings(seed, &installed_fonts(), &names);
    drawn.accent = drawn.accent.filter(|_| !found.accent.is_empty());
    drawn.base = drawn.base.filter(|_| !found.base.is_empty());
    drawn.font = drawn.font.filter(|_| !found.font.is_empty());
    drawn.radius = drawn.radius.filter(|_| !found.radius.is_empty());
    drawn.mode = current.mode;
    drawn.theme.clone_from(&current.theme);
    drawn.radius_shape = current.radius_shape;
    let mut next = state.history().clone();
    next.commit(drawn);
    change(s, RANDOM, next)
}

fn name_of(p: &Value) -> Result<String> {
    match get(p, "name") {
        Some(Value::String(name)) if !name.trim().is_empty() => {
            let name = name.trim();
            if name.chars().count() > MAX_NAME_CHARS || name.chars().any(char::is_control) {
                return Err(bad(SAVE, format!("`name` must be at most {MAX_NAME_CHARS} characters without control characters")));
            }
            Ok(name.to_string())
        }
        _ => Err(bad(SAVE, "`name` must be a non-empty string")),
    }
}

fn collection_named<'a>(doc: &'a Document, name: &str) -> Option<&'a figcraft_doc::Collection> {
    doc.variables.collections.iter().find(|c| c.name == name)
}

fn save_theme(s: &mut Session, p: &Value) -> Result<Value> {
    ensure_ready(s, SAVE)?;
    let name = name_of(p)?;
    let (update, apply) = (flag(p, SAVE, "update", false)?, flag(p, SAVE, "apply", false)?);
    let state = &s.capture.playground;
    let settings = state.history().current().clone();
    let (Some(target), Some(chosen)) = (state.target(), state.theme_of(&settings).map(|c| c.name.clone())) else {
        return Err(bad(SAVE, NOT_OPEN));
    };
    if s.docs.get(s.active).is_none_or(|ds| ds.uid != target.uid) {
        return Err(bad(SAVE, "switch to the document the playground shows first"));
    }
    let was_stale = s.playground_stale();
    let doc = s.doc()?;
    let source =
        collection_named(doc, &chosen).ok_or_else(|| bad(SAVE, format!("the theme \"{}\" is no longer in the document", shown(&chosen))))?.clone();
    let written = rewrites(doc, &source, &settings).map_err(|cause| bad(SAVE, cause.to_string()))?;
    let saved = if update {
        if name != chosen {
            return Err(bad(SAVE, format!("update rewrites the chosen theme \"{}\": `name` must be its name", shown(&chosen))));
        }
        if written.is_empty() {
            return Err(bad(SAVE, "no setting to save: change a setting first"));
        }
        write_into(s.doc_mut()?, &written);
        source.id.clone()
    } else {
        if collection_named(doc, &name).is_some() {
            return Err(bad(SAVE, format!("a collection named \"{}\" already exists", shown(&name))));
        }
        copy_with(s.doc_mut()?, &source.id, &name, &written).ok_or_else(|| bad(SAVE, "the chosen theme is no longer in the document"))?
    };
    if apply {
        super::project::update(s, &json!({"appliedTheme": name}))?;
    }
    show_saved_theme(s, was_stale);
    Ok(json!({"collection": saved, "name": name}))
}

/// The playground's document takes the saved theme, so the menu lists it without a refresh; a document
/// that had already changed otherwise stays stale.
fn show_saved_theme(s: &mut Session, was_stale: bool) {
    let Some(doc) = s.docs.get(s.active).map(|ds| std::sync::Arc::clone(&ds.doc)) else { return };
    if let Some(target) = s.capture.playground.target_mut().filter(|_| !was_stale) {
        target.doc = doc;
    }
}

fn code(s: &mut Session, _: &Value) -> Result<Value> {
    s.playground_sync();
    let state = &s.capture.playground;
    let target = state.target().ok_or_else(|| bad(CODE, NOT_OPEN))?;
    let prepared = state.prepare(CODE, state.history().current())?;
    Ok(json!({"overrides": prepared.overrides(), "template": target.composed.html, "notes": target.composed.notes}))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::playground_support::{Bench, Library};

    fn must(bench: &mut Bench, command: &str, params: Value) -> Value {
        bench.run(command, &params).unwrap_or_else(|error| panic!("{command} {params}: {error}"))
    }

    #[test]
    fn state_reports_the_settings_the_themes_and_nothing_while_closed() {
        let mut closed = Bench::new("settings-state-closed", Library::Full);
        let state = closed.state();
        assert_eq!((state["themes"].clone(), state["settings"].clone(), state["canUndo"].clone()), (json!([]), Value::Null, json!(false)));

        let mut open = Bench::ready("settings-state-open");
        must(&mut open, "playground.set", json!({"font": "Georgia", "radius": "full"}));
        let state = open.state();
        assert_eq!(state["applied"], json!("Theme"));
        assert!(state["themes"].as_array().is_some_and(|names| names.contains(&json!("Theme"))), "{state}");
        assert_eq!((state["settings"]["font"].clone(), state["settings"]["radius"].clone()), (json!("Georgia"), json!("full")));
        assert_eq!(state["effective"]["font"], json!("Georgia"));
        assert_eq!(state["canUndo"], json!(true));
        assert!(state["missing"].as_array().is_some_and(|names| names.contains(&json!("Chart"))), "{state}");
    }

    #[test]
    fn random_leaves_out_what_the_theme_has_no_token_for() {
        let mut bench = Bench::new("settings-random-neutral", Library::Full);
        must(&mut bench, "variable.createCollection", json!({"name": "Neutre", "modes": ["Light", "Dark"]}));
        let values = json!({"Light": "#f0f0f0", "Dark": "#202020"});
        must(&mut bench, "variable.create", json!({"collection": "Neutre", "name": "couleur/fond", "type": "color", "values": values}));
        must(&mut bench, "playground.open", json!({}));
        bench.pump();
        must(&mut bench, "playground.set", json!({"theme": "Neutre"}));

        let drawn = must(&mut bench, "playground.random", json!({"seed": 3}));

        assert!(drawn["settings"]["accent"].is_null() && drawn["settings"]["font"].is_null() && drawn["settings"]["radius"].is_null(), "{drawn}");
        assert!(drawn["settings"]["base"].is_object(), "{drawn}");
        assert_eq!(drawn["settings"]["theme"], json!("Neutre"));
    }

    #[test]
    fn saving_over_a_theme_other_than_the_chosen_one_is_refused() {
        let mut bench = Bench::ready("settings-update-name");
        must(&mut bench, "playground.set", json!({"accent": "#2563eb"}));
        let before = bench.document_text();

        let refused = bench.run("playground.saveTheme", &json!({"name": "Primitives", "update": true}));

        assert!(refused.is_err());
        assert_eq!(bench.document_text(), before);
    }
}
