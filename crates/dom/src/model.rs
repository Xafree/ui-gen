use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    #[serde(default)]
    pub source: Option<Source>,
    pub viewport: Viewport,
    pub root: DomNode,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Source {
    pub package: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub component: String,
    #[serde(default)]
    pub props: serde_json::Value,
    #[serde(default)]
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
    pub device_scale_factor: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DomNode {
    Element(Element),
    Text(TextRun),
    Svg(SvgNode),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct DomBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Element {
    pub tag: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default, rename = "className")]
    pub class_name: Option<String>,
    #[serde(rename = "box")]
    pub r#box: DomBox,
    #[serde(default)]
    pub style: BTreeMap<String, String>,
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    #[serde(default)]
    pub animations: Vec<DomAnimation>,
    #[serde(default)]
    pub children: Vec<DomNode>,
}

/// A CSS animation running on an element; `iterations` is `None` when it never ends.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "RawAnimation")]
pub struct DomAnimation {
    pub name: String,
    pub duration_ms: f64,
    pub easing: String,
    pub iterations: Option<f64>,
    pub properties: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawIterations {
    Count(f64),
    Named(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAnimation {
    #[serde(default)]
    name: String,
    #[serde(default)]
    duration_ms: Option<f64>,
    #[serde(default)]
    easing: String,
    #[serde(default)]
    iterations: Option<RawIterations>,
    #[serde(default)]
    properties: Vec<String>,
}

impl TryFrom<RawAnimation> for DomAnimation {
    type Error = String;

    fn try_from(raw: RawAnimation) -> Result<Self, Self::Error> {
        let Some(duration_ms) = raw.duration_ms.filter(|duration| duration.is_finite() && *duration >= 0.0) else {
            return Err("animation durationMs must be a finite number, 0 or more".to_owned());
        };
        let iterations = match raw.iterations {
            None => Some(1.0),
            Some(RawIterations::Count(count)) if count.is_finite() && count >= 0.0 => Some(count),
            Some(RawIterations::Named(name)) if name == "infinite" => None,
            Some(_) => return Err("animation iterations must be a number, 0 or more, or \"infinite\"".to_owned()),
        };
        Ok(Self { name: raw.name, duration_ms, easing: raw.easing, iterations, properties: raw.properties })
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TextRun {
    pub text: String,
    #[serde(rename = "box")]
    pub r#box: DomBox,
    #[serde(default, rename = "lineBox")]
    pub line_box: Option<DomBox>,
    #[serde(default)]
    pub baseline: Option<f64>,
    #[serde(default)]
    pub style: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SvgNode {
    pub markup: String,
    #[serde(rename = "box")]
    pub r#box: DomBox,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub animations: Vec<DomAnimation>,
}
