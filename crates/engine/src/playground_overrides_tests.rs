//! Playground settings become token overrides (E7-2.2 to E7-2.7): the Accent, Base, Font and
//! Radius settings rewrite the targets of a synthetic theme in each mode, and `overrides_css`
//! writes them under the selectors the page fixes.

use figcraft_color::{Color, Oklch};
use figcraft_doc::VariableValue;

use crate::codegen::LitNames;
use crate::lit_export::{TokenOptions, tokens_css};
use crate::playground::{Neutral, Overrides, RadiusPreset, SettingError, Settings, effective, overrides, overrides_css};
use crate::playground_targets_tests::{
    DARK, Fixture, LIGHT, color, fixture, fixture_from, fixture_with, fixture_without, themed_color, themed_number,
};

const PREFIX: &str = "ui";
const LIGHT_SELECTOR: &str = ":root[data-theme=\"light\"]";
const DARK_SELECTOR: &str = ":root[data-theme=\"dark\"]";

fn describe(error: &SettingError) -> String {
    match error {
        SettingError::InvalidFont(font) => format!("InvalidFont({font})"),
        SettingError::NoTarget(setting) => format!("NoTarget({setting})"),
        _ => "InvalidHue or InvalidTint".to_string(),
    }
}

fn run(f: &Fixture, settings: &Settings) -> Result<Overrides, SettingError> {
    overrides(&f.doc, &f.theme, &LitNames::new(&f.doc, PREFIX), settings)
}

fn applied(f: &Fixture, settings: &Settings) -> Overrides {
    run(f, settings).unwrap_or_else(|e| panic!("overrides refused: {}", describe(&e)))
}

fn refusal(f: &Fixture, settings: &Settings) -> SettingError {
    match run(f, settings) {
        Err(e) => e,
        Ok(o) => panic!("expected a refusal, got {} light and {} dark overrides", o.light.len(), o.dark.len()),
    }
}

fn css_name(f: &Fixture, variable: &str) -> String {
    LitNames::new(&f.doc, PREFIX).name(f.var(variable))
}

fn names_of(list: &[(String, String)]) -> Vec<String> {
    list.iter().map(|(name, _)| name.clone()).collect()
}

fn names_for(f: &Fixture, variables: &[&str]) -> Vec<String> {
    variables.iter().map(|v| css_name(f, v)).collect()
}

fn value_of<'a>(list: &'a [(String, String)], name: &str) -> &'a str {
    list.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str()).unwrap_or_else(|| panic!("no override for {name} in {list:?}"))
}

fn parse(value: &str) -> Color {
    Color::parse_css(value).unwrap_or_else(|| panic!("not a colour: {value}"))
}

fn px(value: &str) -> f64 {
    value.strip_suffix("px").and_then(|n| n.parse().ok()).unwrap_or_else(|| panic!("not a pixel length: {value}"))
}

fn original(f: &Fixture, variable: &str, mode: &str) -> Color {
    let v = f.var(variable);
    match v.values.get(mode) {
        Some(VariableValue::Color(c)) => *c,
        Some(VariableValue::Alias(target)) => match f.doc.variables.resolve(target, &Default::default()) {
            Some(VariableValue::Color(c)) => c,
            other => panic!("{variable} does not resolve to a colour: {other:?}"),
        },
        other => panic!("{variable} has no colour in {mode}: {other:?}"),
    }
}

fn recolor(source: Color, chroma: f32, hue: f32) -> Color {
    Oklch { l: Oklch::from_color(source).l, c: chroma, h: hue }.to_color().with_alpha(source.a)
}

fn close(a: Color, b: Color) -> bool {
    a.to_rgba8().iter().zip(b.to_rgba8()).all(|(x, y)| x.abs_diff(y) <= 2)
}

fn assert_close(what: &str, got: &str, want: Color) {
    let got = parse(got);
    assert!(close(got, want), "{what}: got {:?}, wanted {:?}", got.to_rgba8(), want.to_rgba8());
}

fn blue() -> Color {
    color("#1E6CFF")
}

fn with_accent(accent: Color) -> Settings {
    Settings { accent: Some(accent), ..Settings::default() }
}

fn with_base(hue: f64, tint: f64) -> Settings {
    Settings { base: Some(Neutral { hue, tint }), ..Settings::default() }
}

fn with_radius(preset: RadiusPreset) -> Settings {
    Settings { radius: Some(preset), ..Settings::default() }
}

fn with_font(font: &str) -> Settings {
    Settings { font: Some(font.to_string()), ..Settings::default() }
}

// covers: E7-2.2
#[test]
fn accent_gives_its_target_the_hue_and_chroma_of_the_chosen_blue_in_each_mode() {
    let f = fixture();
    let k = Oklch::from_color(blue());
    let o = applied(&f, &with_accent(blue()));
    let name = css_name(&f, "couleur/primaire");
    assert_eq!(names_of(&o.light), vec![name.clone()]);
    assert_eq!(names_of(&o.dark), vec![name.clone()]);
    let (light, dark) = (original(&f, "couleur/primaire", LIGHT), original(&f, "couleur/primaire", DARK));
    assert_close("light", value_of(&o.light, &name), recolor(light, k.c, k.h));
    assert_close("dark", value_of(&o.dark, &name), recolor(dark, k.c, k.h));
    assert_ne!(value_of(&o.light, &name), value_of(&o.dark, &name), "each mode keeps its own lightness");
}

// covers: E7-2.2
#[test]
fn accent_never_touches_text_or_contrast_tokens() {
    let f = fixture();
    let o = applied(&f, &with_accent(blue()));
    assert!(!o.light.is_empty() && !o.dark.is_empty(), "the accent has a target");
    for untouched in ["couleur/primaire/texte", "couleur/primaire-contraste", "couleur/succes"] {
        let name = css_name(&f, untouched);
        assert!(!names_of(&o.light).contains(&name) && !names_of(&o.dark).contains(&name), "{untouched} was overridden");
    }
}

// covers: E7-2.2
#[test]
fn accent_keeps_the_alpha_of_each_token() {
    let f = fixture_from(vec![themed_color("couleur/primaire", color("#E5532D").with_alpha(0.5), color("#FF7A59").with_alpha(0.25))]);
    let k = Oklch::from_color(blue());
    let o = applied(&f, &with_accent(blue()));
    let name = css_name(&f, "couleur/primaire");
    assert_close("light", value_of(&o.light, &name), recolor(color("#E5532D").with_alpha(0.5), k.c, k.h));
    assert_close("dark", value_of(&o.dark, &name), recolor(color("#FF7A59").with_alpha(0.25), k.c, k.h));
}

// covers: E7-2.2
#[test]
fn accent_chroma_comes_down_into_the_srgb_gamut_and_keeps_lightness_and_hue() {
    let f = fixture();
    let yellow = color("#FFFF00");
    let k = Oklch::from_color(yellow);
    let o = applied(&f, &with_accent(yellow));
    let name = css_name(&f, "couleur/primaire");
    let source = original(&f, "couleur/primaire", LIGHT);
    let got = parse(value_of(&o.light, &name));
    assert!(close(got, recolor(source, k.c, k.h)), "got {:?}", got.to_rgba8());
    let result = Oklch::from_color(got);
    assert!(result.c < k.c - 0.01, "chroma {} was not reduced from {}", result.c, k.c);
    assert!((result.l - Oklch::from_color(source).l).abs() < 0.03, "lightness {} drifted", result.l);
    assert!((result.h - k.h).abs() < 8.0, "hue {} drifted from {}", result.h, k.h);
}

// covers: E7-2.3
#[test]
fn base_tints_only_the_neutrals_keeping_lightness_and_alpha() {
    let f = fixture();
    for (hue, tint) in [(60.0, 1.0), (250.0, 0.5)] {
        let o = applied(&f, &with_base(hue, tint));
        let targets = ["couleur/fond", "couleur/bordure", "couleur/carte"];
        assert_eq!(names_of(&o.light), names_for(&f, &targets), "hue {hue}, tint {tint}");
        assert_eq!(names_of(&o.dark), names_for(&f, &targets), "hue {hue}, tint {tint}");
        for variable in targets {
            let name = css_name(&f, variable);
            let chroma = (tint * 0.04) as f32;
            assert_close(variable, value_of(&o.light, &name), recolor(original(&f, variable, LIGHT), chroma, hue as f32));
            assert_close(variable, value_of(&o.dark, &name), recolor(original(&f, variable, DARK), chroma, hue as f32));
        }
    }
}

// covers: E7-2.3
#[test]
fn base_keeps_the_alpha_of_a_neutral() {
    let f = fixture_from(vec![themed_color("couleur/voile", color("#808080").with_alpha(0.4), color("#C0C0C0").with_alpha(0.4))]);
    let o = applied(&f, &with_base(30.0, 1.0));
    let name = css_name(&f, "couleur/voile");
    assert_close("light", value_of(&o.light, &name), recolor(color("#808080").with_alpha(0.4), 0.04, 30.0));
}

// covers: E7-2.3
#[test]
fn a_tint_of_zero_gives_exact_greys() {
    let f = fixture();
    let o = applied(&f, &with_base(123.0, 0.0));
    for list in [&o.light, &o.dark] {
        assert_eq!(list.len(), 3);
        for (name, value) in list.iter() {
            let [r, g, b, _] = parse(value).to_rgba8();
            assert!(r == g && g == b, "{name}: {value} is not a grey");
        }
    }
}

// covers: E7-2.3
#[test]
fn base_leaves_the_accent_white_and_black_alone() {
    let f = fixture();
    let o = applied(&f, &with_base(200.0, 0.8));
    assert!(!o.light.is_empty() && !o.dark.is_empty(), "the base has targets");
    for untouched in ["couleur/primaire", "couleur/primaire/texte", "couleur/primaire-contraste", "couleur/blanc", "couleur/noir", "couleur/succes"] {
        let name = css_name(&f, untouched);
        assert!(!names_of(&o.light).contains(&name) && !names_of(&o.dark).contains(&name), "{untouched} was overridden");
    }
}

// covers: E7-2.4
#[test]
fn radius_presets_scale_every_radius_token_in_each_mode_to_the_half_pixel() {
    let f = fixture_with(vec![themed_number("rayon/carte", 5.3, 5.3, "CORNER_RADIUS")]);
    let table = [
        (RadiusPreset::Zero, [0.0, 0.0, 0.0]),
        (RadiusPreset::Small, [4.0, 5.0, 2.5]),
        (RadiusPreset::Large, [12.0, 15.0, 8.0]),
        (RadiusPreset::Full, [9999.0, 9999.0, 9999.0]),
    ];
    let (button, card) = (css_name(&f, "rayon/bouton"), css_name(&f, "rayon/carte"));
    for (preset, [button_light, button_dark, card_value]) in table {
        let o = applied(&f, &with_radius(preset));
        assert_eq!(names_of(&o.light), vec![button.clone(), card.clone()]);
        assert_eq!(names_of(&o.dark), vec![button.clone(), card.clone()]);
        assert_eq!(px(value_of(&o.light, &button)), button_light);
        assert_eq!(px(value_of(&o.dark, &button)), button_dark);
        assert_eq!(px(value_of(&o.light, &card)), card_value);
        assert_eq!(px(value_of(&o.dark, &card)), card_value);
    }
}

// covers: E7-2.4
#[test]
fn the_medium_radius_leaves_the_theme_values_alone() {
    let f = fixture();
    let o = applied(&f, &with_radius(RadiusPreset::Medium));
    assert!(o.light.is_empty() && o.dark.is_empty());
    assert_eq!(overrides_css(&o), "");
}

// covers: E7-2.4
#[test]
fn the_radius_setting_touches_no_other_number() {
    let f = fixture_with(vec![themed_number("opacite/voile", 0.5, 0.5, "OPACITY")]);
    let o = applied(&f, &with_radius(RadiusPreset::Large));
    assert_eq!(names_of(&o.light), names_for(&f, &["rayon/bouton"]));
}

// covers: E7-2.5
#[test]
fn the_font_is_written_as_a_css_string_in_each_mode() {
    let f = fixture();
    let name = css_name(&f, "typo/police-famille");
    let longest = "a".repeat(64);
    for font in ["Georgia", "Segoe UI", "system-ui", longest.as_str()] {
        let o = applied(&f, &with_font(font));
        assert_eq!(names_of(&o.light), vec![name.clone()], "{font}");
        assert_eq!(value_of(&o.light, &name), format!("\"{font}\""));
        assert_eq!(value_of(&o.dark, &name), format!("\"{font}\""));
    }
}

// covers: E7-2.5
#[test]
fn an_unsafe_font_is_refused_and_nothing_is_produced() {
    let f = fixture();
    let too_long = "a".repeat(65);
    let refused = [
        "x; } body { display:none",
        "Inter\nbody",
        "url(https://example.test/f.woff)",
        "Inter\\",
        "a{b",
        "a}b",
        "a;b",
        "a(b",
        "a)b",
        too_long.as_str(),
    ];
    for font in refused {
        let settings = Settings { accent: Some(blue()), font: Some(font.to_string()), ..Settings::default() };
        match refusal(&f, &settings) {
            SettingError::InvalidFont(_) => {}
            other => panic!("{font:?}: expected InvalidFont, got {}", describe(&other)),
        }
    }
}

fn full_settings() -> Settings {
    Settings {
        accent: Some(blue()),
        base: Some(Neutral { hue: 200.0, tint: 0.5 }),
        font: Some("Georgia".to_string()),
        radius: Some(RadiusPreset::Large),
        ..Settings::default()
    }
}

fn flat(text: &str) -> String {
    text.split_whitespace().collect()
}

// covers: E7-2.6
#[test]
fn overrides_css_writes_each_mode_under_its_selector_with_the_names_of_tokens_css() {
    let f = fixture();
    let o = applied(&f, &full_settings());
    let css = flat(&overrides_css(&o));
    let (light_at, dark_at) = (css.find(&flat(LIGHT_SELECTOR)), css.find(&flat(DARK_SELECTOR)));
    let (light_at, dark_at) = (light_at.expect("light selector"), dark_at.expect("dark selector"));
    assert!(light_at < dark_at, "light block comes first: {css}");
    let (light_part, dark_part) = (&css[light_at..dark_at], &css[dark_at..]);
    for (part, list) in [(light_part, &o.light), (dark_part, &o.dark)] {
        for (name, value) in list {
            assert!(part.contains(&format!("{name}:{};", flat(value))), "{name} missing in {part}");
        }
    }
    let tokens = tokens_css(&f.doc, &[&f.theme], &TokenOptions { prefix: PREFIX.to_string(), light: None, dark: None });
    for (name, _) in o.light.iter().chain(&o.dark) {
        assert!(tokens.declared.contains(name), "{name} is not declared by tokens.css");
    }
    assert!(tokens.text.contains("[data-theme=\"dark\"]") && css.contains("[data-theme=\"dark\"]"));
}

// covers: E7-2.6
#[test]
fn overrides_follow_the_order_of_the_variables_and_are_stable() {
    let f = fixture();
    let expected = names_for(&f, &["couleur/primaire", "couleur/fond", "couleur/bordure", "couleur/carte", "typo/police-famille", "rayon/bouton"]);
    let (first, second) = (applied(&f, &full_settings()), applied(&f, &full_settings()));
    assert_eq!(names_of(&first.light), expected);
    assert_eq!(names_of(&first.dark), expected);
    assert_eq!(overrides_css(&first), overrides_css(&second));
}

// covers: E7-2.6
#[test]
fn no_setting_gives_an_empty_text() {
    let f = fixture();
    let o = applied(&f, &Settings::default());
    assert!(o.light.is_empty() && o.dark.is_empty());
    assert_eq!(overrides_css(&o), "");
    assert_eq!(overrides_css(&Overrides { light: Vec::new(), dark: Vec::new() }), "");
}

// covers: E7-2.7
#[test]
fn a_setting_without_a_target_is_refused_and_names_the_setting() {
    let accent_less = ["couleur/primaire", "couleur/primaire/texte", "couleur/primaire-contraste"];
    let cases = [
        ("accent", fixture_without(&accent_less), with_accent(blue())),
        ("base", fixture_without(&["couleur/fond", "couleur/bordure", "couleur/carte"]), with_base(10.0, 0.5)),
        ("font", fixture_without(&["typo/police-famille"]), with_font("Georgia")),
        ("radius", fixture_without(&["rayon/bouton"]), with_radius(RadiusPreset::Large)),
    ];
    for (setting, f, settings) in cases {
        match refusal(&f, &settings) {
            SettingError::NoTarget(named) => assert_eq!(named, setting),
            other => panic!("{setting}: expected NoTarget, got {}", describe(&other)),
        }
    }
}

// covers: E7-2.7
#[test]
fn a_hue_or_tint_that_is_not_finite_or_out_of_bounds_is_refused() {
    let f = fixture();
    for hue in [f64::NAN, f64::INFINITY, -1.0, 361.0] {
        assert!(matches!(refusal(&f, &with_base(hue, 0.5)), SettingError::InvalidHue), "hue {hue}");
    }
    for tint in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(matches!(refusal(&f, &with_base(10.0, tint)), SettingError::InvalidTint), "tint {tint}");
    }
}

// covers: E7-2.7
#[test]
fn effective_gives_the_theme_values_when_nothing_is_set() {
    let f = fixture();
    let shown = effective(&f.doc, &f.theme, &Settings::default());
    assert_eq!(shown.accent, Some(color("#E5532D")));
    assert_eq!(shown.font.as_deref(), Some("Inter"));
    assert_eq!(shown.radius, Some(8.0));
}

// covers: E7-2.7
#[test]
fn effective_gives_the_values_that_are_set() {
    let f = fixture();
    let settings = Settings { accent: Some(blue()), font: Some("Georgia".to_string()), ..Settings::default() };
    let shown = effective(&f.doc, &f.theme, &settings);
    assert_eq!(shown.accent, Some(blue()));
    assert_eq!(shown.font.as_deref(), Some("Georgia"));
}
