//! Choosing the installed font a page's text is drawn with: the family Chromium used, or the first
//! of the stack that is installed, and among its styles the one nearest the weight wanted.

use figcraft_doc::style_weight;

use super::capped;
use super::css::split_outside;

const SYSTEM_UI_KEYWORDS: [&str; 3] = ["system-ui", "-apple-system", "blinkmacsystemfont"];

const fn platform_ui_font() -> &'static str {
    if cfg!(windows) {
        "Segoe UI"
    } else if cfg!(target_os = "macos") {
        "SF Pro"
    } else {
        "Noto Sans"
    }
}

pub(super) fn stack_families(stack: &str) -> Vec<&str> {
    split_outside(stack, |c| c == ',')
        .into_iter()
        .map(|family| family.trim_matches(|c: char| matches!(c, '"' | '\'') || c.is_whitespace()))
        .filter(|family| !family.is_empty())
        .collect()
}

pub(super) fn is_installed(family: &str) -> bool {
    figcraft_text::has_family(family)
}

const MAX_STYLE_WORDS: usize = 3;

/// A platform font name like "Segoe UI Semibold" is a face of an installed family: its family and the style it names.
pub(super) fn face_of_platform_name(name: &str) -> Option<(String, String)> {
    let words: Vec<&str> = name.split_whitespace().collect();
    (1..=MAX_STYLE_WORDS.min(words.len().saturating_sub(1))).find_map(|style_words| {
        let (family, style) = words.split_at(words.len() - style_words);
        let family = family.join(" ");
        if !is_installed(&family) {
            return None;
        }
        let wanted = squashed(&style.join(" "));
        figcraft_text::family_styles(&family).into_iter().find(|candidate| squashed(candidate) == wanted).map(|found| (capped(&family), found))
    })
}

fn squashed(style: &str) -> String {
    style.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// The first family of `stack` that is installed; the system-ui keywords stand for the interface font of the platform.
pub(super) fn installed_family(stack: &str) -> Option<String> {
    stack_families(stack).into_iter().find_map(|family| {
        let family = if SYSTEM_UI_KEYWORDS.iter().any(|keyword| family.eq_ignore_ascii_case(keyword)) { platform_ui_font() } else { family };
        is_installed(family).then(|| capped(family))
    })
}

/// The style of `family` nearest `weight`, on the same slant when the family has one: a weight is never synthesised.
pub(super) fn installed_style(family: &str, weight: u16, italic: bool) -> Option<String> {
    let styles: Vec<(String, u16, bool)> = figcraft_text::family_styles(family)
        .into_iter()
        .map(|style| {
            let lower = style.to_ascii_lowercase();
            let slanted = lower.contains("italic") || lower.contains("oblique");
            let weight = style_weight(&style);
            (style, weight, slanted)
        })
        .collect();
    let same_slant: Vec<&(String, u16, bool)> = styles.iter().filter(|style| style.2 == italic).collect();
    let pool = if same_slant.is_empty() { styles.iter().collect() } else { same_slant };
    let weights: Vec<u16> = pool.iter().map(|style| style.1).collect();
    let chosen = nearest_weight(&weights, weight);
    pool.into_iter().find(|style| style.1 == chosen).map(|style| style.0.clone())
}

/// The weight among `available` that CSS font matching picks for `wanted`; `wanted` itself when there is none.
pub fn nearest_weight(available: &[u16], wanted: u16) -> u16 {
    if available.contains(&wanted) {
        return wanted;
    }
    let lighter = available.iter().copied().filter(|weight| *weight < wanted).max();
    let heavier = available.iter().copied().filter(|weight| *weight > wanted).min();
    let preferred = if wanted < 400 {
        [lighter, heavier]
    } else if wanted > 500 {
        [heavier, lighter]
    } else {
        [available.iter().copied().filter(|weight| (wanted..=500).contains(weight)).min().or(lighter), heavier]
    };
    preferred.into_iter().flatten().next().unwrap_or(wanted)
}
