//! `uigen.importComponent`: checks the request, asks the capture service to inspect the package and
//! answers with the job at once. The variants are rendered and written when `Session::pump_capture`
//! brings the results back (`crate::uigen_import`).

use std::path::Path;

use figcraft_project::{Source, WORK_DIR, npm};
use serde_json::{Value, json};

use super::CommandSpec;
use super::project_sources::{absolute_root, binding};
use crate::file_access::{self, Access};
use crate::params::{f64_of, get};
use crate::uigen_import::{self, Destination, MAX_RENDERS, Request, State, Theme, render_count};
use crate::{Result, Session, bad, spec};

const IMPORT: &str = uigen_import::COMMAND;
const CAPTURES_DIR: &str = "captures";
const MAX_NAME_CHARS: usize = 128;
const MAX_LIST: usize = 256;
const MAX_PAGE_NAME_CHARS: usize = 200;
const DEFAULT_STATES: usize = 5;

pub fn specs() -> Vec<CommandSpec> {
    vec![spec!(
        IMPORT,
        "Import component",
        "{package (a source of the library project), component, styleProp? (default \"variant\"), sizeProp? (default \"size\"), styles? [values of styleProp], sizes? [values of sizeProp], states? [Default|Hover|Focus|Pressed|Disabled|Loading], themes? [light|dark] (default both), parent? (a frame or page id; default the current page), x?, y?, stage? (default false: write into the import page \"- Imported · <package> <version>\" instead of the current page; not with parent, x or y), animations? (default true: false leaves out the transitions between states and the ripple and spinner marks)}  asks the capture service to render the component in every style, size, state and theme and returns {job} at once; when all the renders are back they become one component set (Style=…, Size=…, State=… variants) in one undo step, with the variable collection \"<package> <version>\". The job's final event carries {set, collection, variants, warnings}, and {page} with stage; one render that fails stops the import and nothing is added",
        true,
        import_component
    )]
}

fn text<'a>(p: &'a Value, key: &str) -> Result<Option<&'a str>> {
    match get(p, key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(bad(IMPORT, format!("`{key}` must be a string"))),
    }
}

fn name_of(p: &Value, key: &str, default: Option<&str>) -> Result<String> {
    let name = match (text(p, key)?, default) {
        (Some(name), _) => name,
        (None, Some(default)) => default,
        (None, None) => return Err(bad(IMPORT, format!("`{key}` is required"))),
    };
    if name.trim().is_empty() || name.chars().count() > MAX_NAME_CHARS || name.chars().any(char::is_control) {
        return Err(bad(IMPORT, format!("`{key}` must be 1 to {MAX_NAME_CHARS} characters without control characters")));
    }
    Ok(name.to_string())
}

fn list<'a>(p: &'a Value, key: &str) -> Result<Option<Vec<&'a Value>>> {
    match get(p, key) {
        None => Ok(None),
        Some(Value::Array(items)) if !items.is_empty() && items.len() <= MAX_LIST => Ok(Some(items.iter().collect())),
        Some(_) => Err(bad(IMPORT, format!("`{key}` must be a list of 1 to {MAX_LIST} entries"))),
    }
}

fn values(p: &Value, key: &str) -> Result<Option<Vec<String>>> {
    let Some(items) = list(p, key)? else { return Ok(None) };
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let value = item.as_str().filter(|value| uigen_import::usable_value(value));
        let value = value
            .ok_or_else(|| bad(IMPORT, format!("`{key}` holds {item}: a value is 1 to 64 characters without comma, \"=\" or control characters")))?;
        if !out.iter().any(|seen| seen == value) {
            out.push(value.to_string());
        }
    }
    Ok(Some(out))
}

fn states(p: &Value) -> Result<Option<Vec<State>>> {
    let Some(items) = list(p, "states")? else { return Ok(None) };
    let mut chosen = Vec::new();
    for item in items {
        let state = item.as_str().and_then(State::parse);
        chosen.push(state.ok_or_else(|| bad(IMPORT, format!("unknown state {item}: use Default, Hover, Focus, Pressed, Disabled or Loading")))?);
    }
    Ok(Some(State::ALL.into_iter().filter(|state| chosen.contains(state)).collect()))
}

fn themes(p: &Value) -> Result<Vec<Theme>> {
    let Some(items) = list(p, "themes")? else { return Ok(Theme::ALL.to_vec()) };
    let mut chosen = Vec::new();
    for item in items {
        let theme = item.as_str().and_then(Theme::parse);
        chosen.push(theme.ok_or_else(|| bad(IMPORT, format!("unknown theme {item}: use light or dark")))?);
    }
    Ok(Theme::ALL.into_iter().filter(|theme| chosen.contains(theme)).collect())
}

fn at(p: &Value) -> Result<Option<(f64, f64)>> {
    let coordinate = |key: &str| match get(p, key) {
        None => Ok(None),
        Some(v) => f64_of(v).map(Some).ok_or_else(|| bad(IMPORT, format!("`{key}` must be a finite number"))),
    };
    match (coordinate("x")?, coordinate("y")?) {
        (None, None) => Ok(None),
        (Some(x), Some(y)) => Ok(Some((x, y))),
        _ => Err(bad(IMPORT, "x and y go together: give both or neither")),
    }
}

fn parent_of(s: &Session, p: &Value) -> Result<figcraft_doc::NodeId> {
    let parent = super::uigen::parent_of(s, IMPORT, p)?;
    if crate::assets::components::inside_instance(s.doc()?, parent) {
        return Err(bad(IMPORT, format!("`parent` {parent} is inside an instance: layers can't be added to it")));
    }
    Ok(parent)
}

fn flag(p: &Value, key: &str) -> Result<Option<bool>> {
    match get(p, key) {
        None => Ok(None),
        Some(Value::Bool(on)) => Ok(Some(*on)),
        Some(_) => Err(bad(IMPORT, format!("`{key}` must be true or false"))),
    }
}

fn staged(p: &Value, source: &Source) -> Result<bool> {
    if !flag(p, "stage")?.unwrap_or(false) {
        return Ok(false);
    }
    if let Some(key) = ["parent", "x", "y"].into_iter().find(|key| get(p, key).is_some()) {
        return Err(bad(IMPORT, format!("`stage` writes into the import page: it can't come with `{key}`")));
    }
    let name = uigen_import::import_page_name(&source.package, &source.version);
    if name.chars().count() > MAX_PAGE_NAME_CHARS {
        return Err(bad(IMPORT, format!("the import page name is more than {MAX_PAGE_NAME_CHARS} characters: \"{name}\"")));
    }
    Ok(true)
}

fn source_of(s: &Session, p: &Value) -> Result<(String, Source, String)> {
    let binding = binding(s, IMPORT)?;
    let package = text(p, "package")?.ok_or_else(|| bad(IMPORT, "package is required"))?;
    npm::validate_package_name(package).map_err(|e| bad(IMPORT, e.to_string()))?;
    let source = binding.config.sources.iter().find(|source| source.package == package && !source.removed);
    let source = source.ok_or_else(|| bad(IMPORT, format!("package \"{package}\" is not a source of this project")))?;
    Ok((absolute_root(IMPORT, &binding.root)?, source.clone(), binding.root.clone()))
}

fn import_component(s: &mut Session, p: &Value) -> Result<Value> {
    let (root, source, project_root) = source_of(s, p)?;
    let stage = staged(p, &source)?;
    let animations = flag(p, "animations")?.unwrap_or(true);
    let component = name_of(p, "component", None)?;
    let style_prop = name_of(p, "styleProp", Some("variant"))?;
    let size_prop = name_of(p, "sizeProp", Some("size"))?;
    if style_prop == size_prop {
        return Err(bad(IMPORT, "`styleProp` and `sizeProp` must name two different props"));
    }
    let request = Request {
        root,
        package: source.package,
        version: source.version,
        component,
        style_prop,
        size_prop,
        styles: values(p, "styles")?,
        sizes: values(p, "sizes")?,
        states: states(p)?,
        themes: themes(p)?,
        animations,
    };
    let renders = render_count(
        request.styles.as_ref().map_or(0, Vec::len),
        request.sizes.as_ref().map_or(0, Vec::len),
        request.states.as_ref().map_or(DEFAULT_STATES, Vec::len),
        request.themes.len(),
    );
    if renders > MAX_RENDERS {
        return Err(bad(
            IMPORT,
            format!(
                "{renders} renders is more than the limit of {MAX_RENDERS} (variants times themes): ask for fewer styles, sizes, states or themes"
            ),
        ));
    }
    let page = s.page()?;
    let (parent, at) = if stage { (page, None) } else { (parent_of(s, p)?, at(p)?) };
    let destination = Destination { stage, uid: s.ds()?.uid, page, parent, at };
    file_access::allow(Path::new(&project_root).join(WORK_DIR).join(CAPTURES_DIR), Access::Write).map_err(|e| bad(IMPORT, e.to_string()))?;
    let job = uigen_import::start(s, request, destination).map_err(|e| bad(IMPORT, e.message))?;
    Ok(json!({"job": job}))
}
