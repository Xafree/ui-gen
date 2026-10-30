//! The templates lay their content out so that nothing lands on its neighbour: no grid track keeps a
//! fixed length when its content is wider, a row of items wraps instead of overflowing into the next
//! column, and the text of a growing item breaks instead of overflowing. The rules are read from the
//! composed page: the templates add no colour and no font.

use crate::lit_export::ExportPlan;
use crate::playground::{Template, compose};

const FLUID_TEMPLATES: [Template; 5] = [Template::Dashboard, Template::Mail, Template::Chat, Template::Finance, Template::Crm];

struct Rule {
    selector: String,
    declarations: Vec<(String, String)>,
}

fn rules(template: Template) -> Vec<Rule> {
    let html = compose(template, &ExportPlan { components: vec![], files: vec![], notes: vec![] }).html;
    let css: String = html.split("<style>").skip(1).filter_map(|part| part.split("</style>").next()).collect();
    css.split('}')
        .filter_map(|rule| {
            let (selector, body) = rule.split_once('{')?;
            let declarations = body
                .split(';')
                .filter_map(|declaration| declaration.split_once(':'))
                .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
                .collect();
            Some(Rule { selector: selector.trim().to_string(), declarations })
        })
        .collect()
}

fn declared<'a>(rule: &'a Rule, name: &str) -> Option<&'a str> {
    rule.declarations.iter().find(|(declared, _)| declared == name).map(|(_, value)| value.as_str())
}

fn tracks(value: &str) -> Vec<String> {
    let mut tracks = vec![String::new()];
    let mut depth = 0;
    for c in value.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c.is_whitespace() && depth == 0 => {
                tracks.push(String::new());
                continue;
            }
            _ => {}
        }
        tracks.last_mut().unwrap().push(c);
    }
    tracks.retain(|track| !track.is_empty());
    tracks
}

fn is_fixed_length(track: &str) -> bool {
    ["px", "em", "rem", "ch"]
        .iter()
        .any(|unit| track.strip_suffix(unit).is_some_and(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit() || c == '.')))
}

// covers: E7-3.5
#[test]
fn no_grid_track_of_a_template_keeps_a_fixed_length_whatever_its_content() {
    for template in FLUID_TEMPLATES {
        for rule in rules(template) {
            let Some(columns) = declared(&rule, "grid-template-columns") else { continue };
            for track in tracks(columns) {
                assert!(!is_fixed_length(&track), "{}: `{}` has a fixed column {track}", template.name(), rule.selector);
            }
        }
    }
}

// covers: E7-3.5
#[test]
fn every_row_of_a_template_wraps_instead_of_overflowing_into_the_next_column() {
    for template in FLUID_TEMPLATES {
        for rule in rules(template).iter().filter(|rule| declared(rule, "display") == Some("flex")) {
            assert_eq!(declared(rule, "flex-wrap"), Some("wrap"), "{}: `{}` is a row that does not wrap", template.name(), rule.selector);
        }
    }
}

// covers: E7-3.5
#[test]
fn a_growing_item_of_a_template_has_a_basis_and_breaks_its_text() {
    for template in FLUID_TEMPLATES {
        for rule in rules(template).iter().filter(|rule| declared(rule, "flex").is_some_and(|flex| flex.starts_with('1'))) {
            let flex = declared(rule, "flex").unwrap();
            assert!(flex.split_whitespace().count() == 3, "{}: `{}` grows from a basis of 0: {flex}", template.name(), rule.selector);
            assert!(declared(rule, "overflow-wrap").is_some(), "{}: `{}` does not break a long word", template.name(), rule.selector);
        }
    }
}

// covers: E7-3.5
#[test]
fn the_templates_still_impose_no_colour_and_no_font() {
    for template in FLUID_TEMPLATES {
        for rule in rules(template) {
            for (name, _) in &rule.declarations {
                assert!(
                    !name.contains("color") && !name.starts_with("font-family") && name != "background",
                    "{}: `{}` imposes {name}",
                    template.name(),
                    rule.selector
                );
            }
        }
    }
}
