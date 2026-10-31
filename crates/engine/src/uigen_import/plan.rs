//! What an import renders: the axes of the set read from `inspect`, the grid they make and the
//! props of the render job behind each cell.

use serde_json::{Map, Value, json};

use super::Request;
use crate::capture::{CaptureError, CaptureErrorKind};

pub const MAX_RENDERS: usize = 256;
pub(super) const MAX_PROPS: usize = 64;
pub(super) const MAX_VALUES: usize = 64;
pub(super) const MAX_VALUE_CHARS: usize = 64;
const LOADING: &str = "loading";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Default,
    Hover,
    Focus,
    Pressed,
    Disabled,
    Loading,
}

impl State {
    pub(crate) const ALL: [Self; 6] = [Self::Default, Self::Hover, Self::Focus, Self::Pressed, Self::Disabled, Self::Loading];

    pub(crate) fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.label() == name)
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Hover => "Hover",
            Self::Focus => "Focus",
            Self::Pressed => "Pressed",
            Self::Disabled => "Disabled",
            Self::Loading => "Loading",
        }
    }

    pub(super) fn render(self) -> &'static str {
        match self {
            Self::Default | Self::Loading => "default",
            Self::Hover => "hover",
            Self::Focus => "focus",
            Self::Pressed => "pressed",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub(crate) const ALL: [Self; 2] = [Self::Light, Self::Dark];

    pub(crate) fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|theme| theme.render() == name)
    }

    pub(crate) fn mode(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub(crate) fn render(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

pub(super) struct Cell {
    pub label: String,
    pub style: Option<String>,
    pub size: Option<String>,
    pub row: usize,
    pub column: usize,
    pub state: State,
    pub props: Value,
}

pub(super) struct Plan {
    pub cells: Vec<Cell>,
    pub warnings: Vec<String>,
    pub rows: usize,
    pub columns: usize,
}

struct Prop {
    name: String,
    kind: String,
    values: Vec<String>,
}

/// Whether `value` can be part of a variant name (`Style=value, Size=…`) and of a render prop.
pub(crate) fn usable_value(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_VALUE_CHARS
        && value.trim() == value
        && !value.contains([',', '='])
        && !value.chars().any(char::is_control)
}

fn refuse(kind: CaptureErrorKind, message: String) -> CaptureError {
    CaptureError { kind, message }
}

fn prop_of(listed: &Value) -> Option<Prop> {
    let name = listed.get("name")?.as_str()?.to_string();
    let kind = listed.get("type")?.as_str()?.to_string();
    let values = listed
        .get("values")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |values| values.iter().filter_map(Value::as_str).take(MAX_VALUES).map(str::to_string).collect());
    Some(Prop { name, kind, values })
}

fn props_of(inspected: &Value, request: &Request) -> Result<Vec<Prop>, CaptureError> {
    let components = inspected.get("components").and_then(Value::as_array);
    let components = components.ok_or_else(|| refuse(CaptureErrorKind::Protocol, "inspect: the result lists no components".to_string()))?;
    let found = components.iter().find(|component| component.get("name").and_then(Value::as_str) == Some(request.component.as_str()));
    let found = found.ok_or_else(|| {
        refuse(CaptureErrorKind::ComponentNotFound, format!("inspect: component \"{}\" not found in {}", request.component, request.package))
    })?;
    let listed = found.get("props").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    Ok(listed.iter().take(MAX_PROPS).filter_map(prop_of).collect())
}

type Axis = (Vec<Option<String>>, Vec<String>);

fn axis(prop: &str, given: Option<&[String]>, props: &[Prop], component: &str) -> Result<Axis, CaptureError> {
    let read = props.iter().find(|candidate| candidate.name == prop && candidate.kind == "enum").map(|candidate| candidate.values.as_slice());
    let listed = props.iter().any(|candidate| candidate.name == prop);
    let chosen: Vec<String> = match (given, read) {
        (Some(given), Some(read)) => {
            if let Some(missing) = given.iter().find(|value| !read.contains(value)) {
                let message = format!("inspect: `{prop}` has no value \"{missing}\" (it has {})", read.join(", "));
                return Err(refuse(CaptureErrorKind::InvalidRequest, message));
            }
            given.to_vec()
        }
        (Some(_), None) if !listed && !props.is_empty() => {
            let known: Vec<&str> = props.iter().map(|candidate| candidate.name.as_str()).collect();
            let message = format!("inspect: {component} has no prop `{prop}` (it has {})", known.join(", "));
            return Err(refuse(CaptureErrorKind::InvalidRequest, message));
        }
        (Some(given), None) => given.to_vec(),
        (None, Some(read)) => read.to_vec(),
        (None, None) => Vec::new(),
    };
    let (usable, unusable): (Vec<String>, Vec<String>) = chosen.into_iter().partition(|value| usable_value(value));
    let warnings = unusable.iter().map(|value| format!("the value \"{value}\" of `{prop}` can't be part of a variant name: left out")).collect();
    Ok((if usable.is_empty() { vec![None] } else { usable.into_iter().map(Some).collect() }, warnings))
}

fn label(style: Option<&String>, size: Option<&String>, state: State) -> String {
    let mut parts = Vec::new();
    if let Some(style) = style {
        parts.push(format!("Style={style}"));
    }
    if let Some(size) = size {
        parts.push(format!("Size={size}"));
    }
    parts.push(format!("State={}", state.label()));
    parts.join(", ")
}

fn render_props(request: &Request, style: Option<&String>, size: Option<&String>, state: State) -> Value {
    let mut props = Map::new();
    if let Some(style) = style {
        props.insert(request.style_prop.clone(), json!(style));
    }
    if let Some(size) = size {
        props.insert(request.size_prop.clone(), json!(size));
    }
    if state == State::Loading {
        props.insert(LOADING.to_string(), json!(true));
    }
    Value::Object(props)
}

/// The grid of variants: states across, styles then sizes down.
pub(super) fn plan(request: &Request, inspected: &Value) -> Result<Plan, CaptureError> {
    let props = props_of(inspected, request)?;
    let (styles, style_warnings) = axis(&request.style_prop, request.styles.as_deref(), &props, &request.component)?;
    let (sizes, size_warnings) = axis(&request.size_prop, request.sizes.as_deref(), &props, &request.component)?;
    let has_loading = props.iter().any(|prop| prop.name == LOADING && prop.kind == "boolean");
    let states: Vec<State> = match &request.states {
        Some(chosen) if chosen.contains(&State::Loading) && !has_loading => {
            let message = format!("inspect: State=Loading needs a boolean `loading` prop and {} has none", request.component);
            return Err(refuse(CaptureErrorKind::InvalidRequest, message));
        }
        Some(chosen) => chosen.clone(),
        None => State::ALL.into_iter().filter(|state| *state != State::Loading || has_loading).collect(),
    };
    let rows = styles.len() * sizes.len();
    let renders = rows * states.len() * request.themes.len();
    if renders > MAX_RENDERS {
        let message = format!("inspect: {renders} renders is more than the limit of {MAX_RENDERS} (variants times themes)");
        return Err(refuse(CaptureErrorKind::InvalidRequest, message));
    }
    let mut cells = Vec::with_capacity(rows * states.len());
    for (style_index, style) in styles.iter().enumerate() {
        for (size_index, size) in sizes.iter().enumerate() {
            for (column, state) in states.iter().enumerate() {
                let (style, size) = (style.as_ref(), size.as_ref());
                cells.push(Cell {
                    label: label(style, size, *state),
                    style: style.cloned(),
                    size: size.cloned(),
                    row: style_index * sizes.len() + size_index,
                    column,
                    state: *state,
                    props: render_props(request, style, size, *state),
                });
            }
        }
    }
    Ok(Plan { cells, warnings: [style_warnings, size_warnings].concat(), rows, columns: states.len() })
}
