//! Playground settings history and chance (E7-2.8, E7-2.9): a stack of at most 100 steps apart from
//! the document's undo, and a seeded draw of settings.

use std::collections::HashSet;

use figcraft_color::{Color, Oklch};

use crate::playground::{History, RadiusPreset, RadiusShape, Settings, random_settings};

fn font(name: &str) -> Settings {
    Settings { font: Some(name.to_string()), ..Settings::default() }
}

fn current_font(history: &History) -> Option<&str> {
    history.current().font.as_deref()
}

fn undo_all(history: &mut History) -> usize {
    let mut steps = 0;
    while history.undo() {
        steps += 1;
        assert!(steps <= 1000, "undo never ends");
    }
    steps
}

// covers: E7-2.8
#[test]
fn a_new_history_has_empty_settings_and_nothing_to_undo_or_redo() {
    let history = History::new();
    assert_eq!(current_font(&history), None);
    assert!(history.current().accent.is_none() && history.current().base.is_none() && history.current().radius.is_none());
    assert!(!history.can_undo() && !history.can_redo());
}

// covers: E7-2.8
#[test]
fn commit_adds_a_step_that_undo_and_redo_walk_back_and_forth() {
    let mut history = History::new();
    history.commit(font("A"));
    history.commit(font("B"));
    assert_eq!(current_font(&history), Some("B"));
    assert!(history.can_undo() && !history.can_redo());
    assert!(history.undo());
    assert_eq!(current_font(&history), Some("A"));
    assert!(history.undo());
    assert_eq!(current_font(&history), None);
    assert!(!history.undo() && !history.can_undo());
    assert!(history.can_redo());
    assert!(history.redo());
    assert_eq!(current_font(&history), Some("A"));
    assert!(history.redo());
    assert_eq!(current_font(&history), Some("B"));
    assert!(!history.redo() && !history.can_redo());
}

// covers: E7-2.8
#[test]
fn preview_replaces_the_current_settings_without_adding_a_step() {
    let mut history = History::new();
    history.commit(font("A"));
    history.preview(font("B1"));
    history.preview(font("B2"));
    assert_eq!(current_font(&history), Some("B2"));
    history.commit(font("B3"));
    assert_eq!(current_font(&history), Some("B3"));
    assert!(history.undo());
    assert_eq!(current_font(&history), Some("A"), "the previews left no step");
    assert!(history.undo());
    assert_eq!(current_font(&history), None);
    assert!(!history.can_undo());
}

// covers: E7-2.8
#[test]
fn a_commit_after_an_undo_drops_the_redo_steps() {
    let mut history = History::new();
    history.commit(font("A"));
    history.commit(font("B"));
    assert!(history.undo());
    assert!(history.can_redo());
    history.commit(font("C"));
    assert!(!history.can_redo() && !history.redo());
    assert_eq!(current_font(&history), Some("C"));
    assert!(history.undo());
    assert_eq!(current_font(&history), Some("A"));
}

// covers: E7-2.8
#[test]
fn revert_goes_back_to_empty_settings_in_one_undoable_step() {
    let mut history = History::new();
    history.commit(Settings { accent: Some(Color::BLACK), radius: Some(RadiusPreset::Large), ..font("A") });
    history.commit(font("B"));
    history.revert();
    let now = history.current();
    assert!(now.font.is_none() && now.accent.is_none() && now.base.is_none() && now.radius.is_none() && now.theme.is_none());
    assert!(history.can_undo());
    assert!(history.undo());
    assert_eq!(current_font(&history), Some("B"), "revert was one step");
    assert!(history.redo());
    assert_eq!(current_font(&history), None);
}

// covers: E7-2.8
#[test]
fn the_history_keeps_100_steps_and_the_oldest_falls() {
    let mut history = History::new();
    for n in 1..=150 {
        history.commit(font(&format!("f{n}")));
    }
    assert_eq!(current_font(&history), Some("f150"));
    let steps = undo_all(&mut history);
    let oldest = current_font(&history).map(str::to_string);
    let kept = [(100, Some("f50".to_string())), (99, Some("f51".to_string()))];
    assert!(kept.contains(&(steps, oldest.clone())), "{steps} steps back to {oldest:?}");
}

fn fingerprint(s: &Settings) -> String {
    let radius = s.radius.as_ref().map(|r| match r {
        RadiusPreset::Zero => 0,
        RadiusPreset::Small => 1,
        RadiusPreset::Medium => 2,
        RadiusPreset::Large => 3,
        RadiusPreset::Full => 4,
    });
    let shape = match s.radius_shape {
        RadiusShape::Round => 0,
        RadiusShape::Squircle => 1,
        RadiusShape::Bevel => 2,
    };
    let base = s.base.as_ref().map(|n| (n.hue, n.tint));
    format!("{:?}|{base:?}|{:?}|{radius:?}|{shape}|{:?}", s.accent, s.font, s.theme)
}

fn fonts() -> Vec<String> {
    ["Inter", "Georgia", "Fira Code"].iter().map(|f| f.to_string()).collect()
}

fn themes() -> Vec<String> {
    ["Theme", "Brand"].iter().map(|t| t.to_string()).collect()
}

// covers: E7-2.9
#[test]
fn the_same_seed_draws_the_same_settings_and_two_seeds_differ() {
    let (fonts, themes) = (fonts(), themes());
    assert_eq!(fingerprint(&random_settings(42, &fonts, &themes)), fingerprint(&random_settings(42, &fonts, &themes)));
    assert_ne!(fingerprint(&random_settings(1, &fonts, &themes)), fingerprint(&random_settings(2, &fonts, &themes)));
    let distinct: HashSet<String> = (1..=32).map(|seed| fingerprint(&random_settings(seed, &fonts, &themes))).collect();
    assert!(distinct.len() >= 30, "only {} different draws out of 32 seeds", distinct.len());
}

// covers: E7-2.9
#[test]
fn a_draw_has_a_vivid_accent_a_light_tint_a_listed_font_and_a_radius() {
    let (fonts, themes) = (fonts(), themes());
    let mut presets = HashSet::new();
    for seed in 0..500 {
        let drawn = random_settings(seed, &fonts, &themes);
        let accent = drawn.accent.unwrap_or_else(|| panic!("seed {seed}: no accent"));
        assert!(Oklch::from_color(accent).c >= 0.079, "seed {seed}: accent chroma {}", Oklch::from_color(accent).c);
        let base = drawn.base.unwrap_or_else(|| panic!("seed {seed}: no base"));
        assert!(base.hue.is_finite() && (0.0..360.0).contains(&base.hue), "seed {seed}: hue {}", base.hue);
        assert!(base.tint.is_finite() && (0.0..=0.6).contains(&base.tint), "seed {seed}: tint {}", base.tint);
        let font = drawn.font.unwrap_or_else(|| panic!("seed {seed}: no font"));
        assert!(fonts.contains(&font), "seed {seed}: font {font}");
        assert!(drawn.theme.as_ref().is_none_or(|t| themes.contains(t)), "seed {seed}: theme {:?}", drawn.theme);
        let radius = drawn.radius.unwrap_or_else(|| panic!("seed {seed}: no radius"));
        presets.insert(fingerprint(&Settings { radius: Some(radius), ..Settings::default() }));
    }
    assert_eq!(presets.len(), 5, "every radius level is drawn");
}

// covers: E7-2.9
#[test]
fn the_engine_adds_no_random_number_crate() {
    let manifest = include_str!("../Cargo.toml");
    let dependencies: &str = manifest.split("[dev-dependencies]").next().unwrap_or(manifest);
    for generator in ["rand", "fastrand", "getrandom", "oorandom", "nanorand", "rand_core", "rand_chacha", "uuid"] {
        let declared = dependencies
            .lines()
            .any(|line| line.trim_start().starts_with(&format!("{generator} ")) || line.trim_start().starts_with(&format!("{generator}=")));
        assert!(!declared, "the engine depends on {generator}");
    }
}
