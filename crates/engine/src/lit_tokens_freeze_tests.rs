//! `variable.export` css, tailwind and dart stay byte for byte what they were before the Lit
//! tokens work split the CSS writer: the strings below were captured from the code as it stood
//! at UIG6.3, before any change to it.

use crate::Session;
use crate::components::tests::{ex, idv};
use serde_json::json;

const CSS: &str = r####"
/* Colors · Light */
:root {
  --colors-brand-500: #3366FF;
  --colors-surface: #FFFFFF;
  --colors-accent: var(--colors-brand-500);
  --brand-primary: #112233;
}
/* Colors · Dark */
[data-colors="Dark"] {
  --colors-brand-500: #6699FF;
  --colors-surface: #111111;
  --colors-accent: var(--colors-brand-500);
  --brand-primary: #112233;
}
/* Metrics · Base */
:root {
  --space-sm: 4px;
  --radius-md: 8px;
  --opacity-muted: 50;
  --label: "Hello";
  --flag: true;
  --note: "x* /y z w q";
}
"####;
const TAILWIND: &str = r####"
@import "tailwindcss";

/* Colors · Light */
@theme static {
  --color-brand-500: #3366FF;
  --color-surface: #FFFFFF;
  --color-accent: var(--color-brand-500);
  --color-primary: #112233;
}
/* Colors · Dark */
[data-colors="Dark"] {
  --color-brand-500: #6699FF;
  --color-surface: #111111;
  --color-accent: var(--color-brand-500);
  --color-primary: #112233;
}
/* Metrics · Base */
@theme static {
  --spacing-sm: 4px;
  --radius-md: 8px;
  --opacity-muted: 50;
  --label: "Hello";
  --flag: true;
  --note: "x* /y z w q";
}
"####;
const DART: &str = r####"
// Variables exported from UIGEN.

import 'dart:ui';

/// Colors · Light
abstract final class Colors {
  static const Color colorsBrand500 = Color(0xFF3366FF);
  static const Color colorsSurface = Color(0xFFFFFFFF);
  static const Color colorsAccent = Color(0xFF3366FF);
  static const Color colorsPrimary = Color(0xFF112233);
}

/// Colors · Dark
abstract final class ColorsDark {
  static const Color colorsBrand500 = Color(0xFF6699FF);
  static const Color colorsSurface = Color(0xFF111111);
  static const Color colorsAccent = Color(0xFF6699FF);
  static const Color colorsPrimary = Color(0xFF112233);
}

/// Metrics · Base
abstract final class Metrics {
  static const double spaceSm = 4;
  static const double radiusMd = 8;
  static const double opacityMuted = 50;
  static const String label = 'Hello';
  static const bool flag = true;
  static const String note = 'x*/y"z\\w\nq';
}
"####;

fn sample() -> Session {
    let mut s = Session::new();
    ex(&mut s, "variable.createCollection", json!({"name": "Colors", "modes": ["Light", "Dark"]}));
    ex(
        &mut s,
        "variable.create",
        json!({"collection": "Colors", "name": "Colors/Brand 500", "type": "color", "values": {"Light": "#3366FF", "Dark": "#6699FF"}}),
    );
    ex(
        &mut s,
        "variable.create",
        json!({"collection": "Colors", "name": "Colors/Surface", "type": "color", "values": {"Light": "#FFFFFF", "Dark": "#111111"}}),
    );
    ex(&mut s, "variable.create", json!({"collection": "Colors", "name": "Colors/Accent", "type": "color", "value": {"alias": "Colors/Brand 500"}}));
    ex(&mut s, "variable.createCollection", json!({"name": "Metrics", "modes": ["Base"]}));
    for (name, value) in [("Space/sm", 4), ("Radius/md", 8), ("Opacity/muted", 50)] {
        ex(&mut s, "variable.create", json!({"collection": "Metrics", "name": name, "type": "number", "value": value}));
    }
    ex(&mut s, "variable.create", json!({"collection": "Metrics", "name": "Label", "type": "string", "value": "Hello"}));
    ex(&mut s, "variable.create", json!({"collection": "Metrics", "name": "Flag", "type": "boolean", "value": true}));
    ex(&mut s, "variable.create", json!({"collection": "Metrics", "name": "Note", "type": "string", "value": "x*/y\"z\\w\nq"}));
    ex(&mut s, "variable.create", json!({"collection": "Colors", "name": "Colors/Primary", "type": "color", "value": "#112233"}));
    ex(&mut s, "variable.update", json!({"variable": "Colors/Primary", "codeSyntax": {"WEB": "var(--brand-primary)"}}));
    let f = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 100})));
    ex(&mut s, "layout.add", json!({"ids": [f]}));
    for (field, variable) in [("gap", "Space/sm"), ("cornerRadius", "Radius/md"), ("opacity", "Opacity/muted"), ("fill", "Colors/Brand 500")] {
        ex(&mut s, "variable.bind", json!({"ids": [f], "field": field, "variable": variable}));
    }
    s
}

fn exported(format: &str) -> String {
    let mut s = sample();
    ex(&mut s, "variable.export", json!({"format": format}))["text"].as_str().unwrap_or_default().to_string()
}

#[test]
fn variable_export_css_is_unchanged() {
    // covers: E6-3.7
    assert_eq!(exported("css"), CSS.trim_start());
}

#[test]
fn variable_export_tailwind_is_unchanged() {
    // covers: E6-3.7
    assert_eq!(exported("tailwind"), TAILWIND.trim_start());
}

#[test]
fn variable_export_dart_is_unchanged() {
    // covers: E6-3.7
    assert_eq!(exported("dart"), DART.trim_start());
}
