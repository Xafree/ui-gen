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

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct DomBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Element {
    pub tag: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(rename = "box")]
    pub r#box: DomBox,
    #[serde(default)]
    pub style: BTreeMap<String, String>,
    #[serde(default)]
    pub children: Vec<DomNode>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TextRun {
    pub text: String,
    #[serde(rename = "box")]
    pub r#box: DomBox,
    #[serde(default)]
    pub style: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SvgNode {
    pub markup: String,
    #[serde(rename = "box")]
    pub r#box: DomBox,
}
