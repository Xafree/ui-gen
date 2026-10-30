//! The playground commands: `open` and `refresh` export the applied theme of a library project into
//! its staging folder and ask the capture service to show the page, `template` swaps the page's
//! model, `input` relays the pointer and keyboard, `state` reports. The images come back through
//! `Session::playground_poll`; the page itself lives in the service.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use figcraft_doc::Document;
use figcraft_project::WORK_DIR;
use serde_json::{Value, json};

use super::CommandSpec;
use super::project_sources::absolute_root;
use crate::capture::unavailable_message;
use crate::file_access::{self, Access};
use crate::lit::{LitOptions, valid_prefix};
use crate::lit_export::{ExportOptions, ExportPlan, Status as ComponentStatus, plan, write};
use crate::params::{f64_of, get};
use crate::playground::{Composed, NO_COMPONENT, Status, Target, Template, Window, compose};
use crate::{Result, Session, bad, spec};

const OPEN: &str = "playground.open";
const REFRESH: &str = "playground.refresh";
const CLOSE: &str = "playground.close";
const TEMPLATE: &str = "playground.template";
const STATE: &str = "playground.state";
const INPUT: &str = "playground.input";
const RESIZE: &str = "playground.resize";
const STAGING: &str = "playground";
const MAX_TAGS: usize = 256;
const WIDTH: (f64, f64, f64) = (320.0, 3840.0, 1280.0);
const HEIGHT: (f64, f64, f64) = (240.0, 2160.0, 800.0);
const SCALE: (f64, f64, f64) = (1.0, 2.0, 1.0);
const MAX_EVENTS: usize = 64;
const MAX_WHEEL: f64 = 4000.0;
const MAX_TEXT_CHARS: usize = 256;
const MAX_SHOWN: usize = 40;
const NOT_IN_PROJECT: &str = "the current file is not in a library project";
pub(super) const NOT_OPEN: &str = "the playground is not open";
const KEYS: [&str; 14] = [
    "Enter",
    "Tab",
    "Escape",
    "Backspace",
    "Delete",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Space",
];
const MODIFIERS: [&str; 3] = ["shift", "ctrl", "alt"];
const BUTTONS: [&str; 3] = ["left", "middle", "right"];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            OPEN,
            "Open playground",
            "{template? (components|dashboard|mail|chat|finance|crm; default components), width? (320 to 3840; default 1280), height? (240 to 2160; default 800), scale? (1 to 2; default 1)} → {job}  exports the applied theme of the library project into .uigen/playground and asks the capture service to show the page; the final event of the job is playground.open. Replaces the playground already open",
            false,
            open
        ),
        spec!(REFRESH, "Refresh playground", "{} → {job}  exports the document the playground shows again and opens the page again", false, refresh),
        spec!(CLOSE, "Close playground", "{}  closes the page", false, close),
        spec!(
            TEMPLATE,
            "Set playground template",
            "{name (components|dashboard|mail|chat|finance|crm)} → {missing: [component names the library lacks]}  changes the model shown without opening the page again",
            false,
            template
        ),
        spec!(
            STATE,
            "Playground state",
            "{} → {status: closed|opening|ready|failed, error, template, stale (the document changed since the export), busy (the capture service has an install, inspect or render to do first), themes: [collection names], applied (the applied theme), settings, effective, targets, canUndo, canRedo, missing: [component names the library lacks]}",
            false,
            state
        ),
        spec!(
            RESIZE,
            "Resize playground",
            "{width? (320 to 3840), height? (240 to 2160), scale? (1 to 2)} (each one kept as it is when absent)  sets the window of the page, which shows at that size",
            false,
            resize
        ),
        spec!(
            INPUT,
            "Send playground input",
            "{events (1 to 64): [{type: move, x, y} | {type: down|up, x, y, button: left|middle|right} | {type: wheel, x, y, dx, dy} | {type: key, key (Enter Tab Escape Backspace Delete ArrowUp ArrowDown ArrowLeft ArrowRight Home End PageUp PageDown Space), mods?: [shift|ctrl|alt]} | {type: text, text}]}  relays the pointer and keyboard to the page; one invalid event refuses the batch",
            false,
            input
        ),
    ]
}

struct Tab {
    uid: u64,
    root: String,
    tag_prefix: String,
    applied_theme: Option<String>,
    doc: Arc<Document>,
}

pub(super) fn shown(name: &str) -> String {
    name.chars().take(MAX_SHOWN).collect::<String>().escape_debug().to_string()
}

fn tab_of(s: &Session, cmd: &str, uid: Option<u64>) -> Result<Tab> {
    let ds = match uid {
        Some(uid) => s.docs.iter().find(|ds| ds.uid == uid).ok_or_else(|| bad(cmd, "the document the playground shows is closed"))?,
        None => s.docs.get(s.active).ok_or_else(|| bad(cmd, NOT_IN_PROJECT))?,
    };
    let binding = ds.project.as_ref().ok_or_else(|| bad(cmd, NOT_IN_PROJECT))?;
    Ok(Tab {
        uid: ds.uid,
        root: binding.root.clone(),
        tag_prefix: binding.config.tag_prefix.clone(),
        applied_theme: binding.config.applied_theme.clone(),
        doc: Arc::clone(&ds.doc),
    })
}

fn template_named(cmd: &str, name: &str) -> Result<Template> {
    Template::parse(name)
        .ok_or_else(|| bad(cmd, format!("unknown template \"{}\": use components, dashboard, mail, chat, finance or crm", shown(name))))
}

fn template_param(p: &Value) -> Result<Template> {
    match get(p, "template") {
        None => Ok(Template::Components),
        Some(Value::String(name)) => template_named(OPEN, name),
        Some(_) => Err(bad(OPEN, "`template` must be a string")),
    }
}

fn bounded(cmd: &str, p: &Value, key: &str, (min, max, default): (f64, f64, f64), whole: bool) -> Result<f64> {
    let Some(value) = get(p, key) else { return Ok(default) };
    f64_of(value)
        .filter(|n| (min..=max).contains(n) && (!whole || n.fract() == 0.0))
        .ok_or_else(|| bad(cmd, format!("`{key}` must be {}a number from {min} to {max}", if whole { "a whole " } else { "" })))
}

fn window_param(cmd: &str, p: &Value, current: Option<Window>) -> Result<Window> {
    let kept = |(min, max, default): (f64, f64, f64), now: Option<f64>| (min, max, now.unwrap_or(default));
    let width = bounded(cmd, p, "width", kept(WIDTH, current.map(|w| f64::from(w.width))), true)?;
    let height = bounded(cmd, p, "height", kept(HEIGHT, current.map(|w| f64::from(w.height))), true)?;
    let scale = bounded(cmd, p, "scale", kept(SCALE, current.map(|w| w.scale)), false)?;
    Ok(Window { width: width as u32, height: height as u32, scale })
}

struct Exported {
    plan: ExportPlan,
    theme: String,
    token_prefix: String,
}

fn export_plan(cmd: &str, tab: &Tab) -> Result<Exported> {
    let variables = &tab.doc.variables;
    let theme = tab.applied_theme.as_deref().and_then(|name| variables.collection(name)).or(variables.collections.first());
    let theme = theme.ok_or_else(|| bad(cmd, "the project has no variable collection to take a theme from"))?;
    let mut options = ExportOptions { collections: Some(vec![theme.id.clone()]), ..ExportOptions::default() };
    if valid_prefix(&tab.tag_prefix) {
        options.lit = LitOptions::new(&tab.tag_prefix);
    }
    let token_prefix = options.lit.token_prefix.clone();
    let exported = plan(&tab.doc, &options);
    if exported.components.is_empty() {
        return Err(bad(cmd, NO_COMPONENT));
    }
    if !exported.ready() {
        return Err(bad(cmd, format!("fix these components first: {}", exported.blockers().join(", "))));
    }
    Ok(Exported { plan: exported, theme: theme.name.clone(), token_prefix })
}

fn staging(root: &str) -> PathBuf {
    Path::new(root).join(WORK_DIR).join(STAGING)
}

fn write_template(dir: &Path, composed: &Composed) -> Result<()> {
    file_access::write(dir.join("template.html"), composed.html.as_bytes())
}

fn start(s: &mut Session, command: &'static str, tab: Tab, template: Template, window: Window) -> Result<Value> {
    if !s.capture.available() {
        return Err(bad(command, unavailable_message()));
    }
    let Exported { plan: mut exported, theme, token_prefix } = export_plan(command, &tab)?;
    let composed = compose(template, &exported);
    let root = absolute_root(command, &tab.root)?;
    let dir = staging(&root);
    file_access::allow(&dir, Access::Write).map_err(|e| bad(command, e.to_string()))?;
    s.playground_close();
    write(&exported, &dir.join("lib").to_string_lossy(), true)?;
    write_template(&dir, &composed)?;
    let tags: Vec<&str> =
        exported.components.iter().filter(|c| c.status == ComponentStatus::Ok).filter_map(|c| c.tag.as_deref()).take(MAX_TAGS).collect();
    let params = json!({"tags": tags, "theme": "light", "width": window.width, "height": window.height, "scale": window.scale});
    exported.files.clear();
    let target = Target { uid: tab.uid, root, doc: tab.doc, template, plan: exported, window, composed, base_theme: theme, token_prefix };
    let job = s.playground_opening(command, target, params).map_err(|e| bad(command, e.message))?;
    Ok(json!({"job": job}))
}

fn open(s: &mut Session, p: &Value) -> Result<Value> {
    let (template, window) = (template_param(p)?, window_param(OPEN, p, None)?);
    let tab = tab_of(s, OPEN, None)?;
    start(s, OPEN, tab, template, window)
}

fn refresh(s: &mut Session, _: &Value) -> Result<Value> {
    s.playground_sync();
    let target = s.capture.playground.target().ok_or_else(|| bad(REFRESH, NOT_OPEN))?;
    let (uid, template, window) = (target.uid, target.template, target.window);
    let tab = tab_of(s, REFRESH, Some(uid))?;
    start(s, REFRESH, tab, template, window)
}

fn resize(s: &mut Session, p: &Value) -> Result<Value> {
    ensure_ready(s, RESIZE)?;
    let current = s.capture.playground.target().map(|target| target.window);
    let window = window_param(RESIZE, p, current)?;
    s.playground_resize(window).map_err(|e| bad(RESIZE, e.message))?;
    super::ok()
}

fn close(s: &mut Session, _: &Value) -> Result<Value> {
    s.playground_close();
    super::ok()
}

fn template(s: &mut Session, p: &Value) -> Result<Value> {
    let Some(Value::String(name)) = get(p, "name") else { return Err(bad(TEMPLATE, "`name` must be a string")) };
    let template = template_named(TEMPLATE, name)?;
    s.playground_sync();
    let state = &s.capture.playground;
    let target = state.target().filter(|_| matches!(state.status(), Status::Opening | Status::Ready)).ok_or_else(|| bad(TEMPLATE, NOT_OPEN))?;
    let composed = compose(template, &target.plan);
    write_template(&staging(&target.root), &composed)?;
    s.playground_set_template().map_err(|e| bad(TEMPLATE, e.message))?;
    let missing = composed.missing.clone();
    if let Some(target) = s.capture.playground.target_mut() {
        target.template = template;
        target.composed = composed;
    }
    Ok(json!({"missing": missing}))
}

fn state(s: &mut Session, _: &Value) -> Result<Value> {
    s.playground_sync();
    let state = &s.capture.playground;
    let (status, error) = match state.status() {
        Status::Closed => ("closed", None),
        Status::Opening => ("opening", None),
        Status::Ready => ("ready", None),
        Status::Failed(message) => ("failed", Some(message.as_str())),
    };
    let template = state.target().map(|target| target.template.name());
    let mut answer = json!({"status": status, "error": error, "template": template, "stale": s.playground_stale(), "busy": s.playground_busy()});
    if let (Some(fields), Value::Object(more)) = (answer.as_object_mut(), super::playground_settings::summary(s)) {
        fields.extend(more);
    }
    Ok(answer)
}

fn coordinate(event: &Value, key: &str) -> std::result::Result<f64, String> {
    event.get(key).and_then(Value::as_f64).ok_or_else(|| format!("`{key}` must be a number"))
}

fn one_of(event: &Value, key: &str, allowed: &[&str]) -> std::result::Result<(), String> {
    match event.get(key).and_then(Value::as_str) {
        Some(value) if allowed.contains(&value) => Ok(()),
        _ => Err(format!("`{key}` must be one of {}", allowed.join(", "))),
    }
}

fn modifiers(event: &Value) -> std::result::Result<(), String> {
    match get(event, "mods") {
        None => Ok(()),
        Some(Value::Array(mods)) if mods.iter().all(|m| m.as_str().is_some_and(|m| MODIFIERS.contains(&m))) => Ok(()),
        Some(_) => Err(format!("`mods` must be a list of {}", MODIFIERS.join(", "))),
    }
}

fn wheel(event: &Value, key: &str) -> std::result::Result<(), String> {
    if coordinate(event, key)?.abs() > MAX_WHEEL {
        return Err(format!("`{key}` must be at most {MAX_WHEEL} either way"));
    }
    Ok(())
}

fn text(event: &Value) -> std::result::Result<(), String> {
    match event.get("text").and_then(Value::as_str) {
        Some(text) if text.chars().count() <= MAX_TEXT_CHARS && !text.chars().any(char::is_control) => Ok(()),
        _ => Err(format!("`text` must be at most {MAX_TEXT_CHARS} characters without control characters")),
    }
}

fn valid_event(event: &Value) -> std::result::Result<(), String> {
    let kind = event.get("type").and_then(Value::as_str).ok_or("an event needs a `type`")?;
    if matches!(kind, "move" | "down" | "up" | "wheel") {
        coordinate(event, "x")?;
        coordinate(event, "y")?;
    }
    match kind {
        "move" => Ok(()),
        "down" | "up" => one_of(event, "button", &BUTTONS),
        "wheel" => wheel(event, "dx").and_then(|()| wheel(event, "dy")),
        "key" => one_of(event, "key", &KEYS).and_then(|()| modifiers(event)),
        "text" => text(event),
        other => Err(format!("unknown event type \"{}\"", shown(other))),
    }
}

pub(super) fn ensure_ready(s: &mut Session, cmd: &str) -> Result<()> {
    s.playground_sync();
    match s.capture.playground.status() {
        Status::Ready => Ok(()),
        Status::Opening => Err(bad(cmd, "the playground is still opening")),
        Status::Closed | Status::Failed(_) => Err(bad(cmd, NOT_OPEN)),
    }
}

fn input(s: &mut Session, p: &Value) -> Result<Value> {
    ensure_ready(s, INPUT)?;
    let events = match get(p, "events") {
        Some(Value::Array(events)) if (1..=MAX_EVENTS).contains(&events.len()) => events,
        _ => return Err(bad(INPUT, format!("`events` must be a list of 1 to {MAX_EVENTS} events"))),
    };
    for (index, event) in events.iter().enumerate() {
        valid_event(event).map_err(|why| bad(INPUT, format!("event {index}: {why}")))?;
    }
    s.playground_input(Value::Array(events.clone())).map_err(|e| bad(INPUT, e.message))?;
    super::ok()
}
