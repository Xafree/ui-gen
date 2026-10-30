use std::collections::BTreeMap;

use figcraft_doc::NodeId;

use super::{Template, compose};
use crate::lit::StateRecipe;
use crate::lit_export::{ExportPlan, PlannedComponent, Status};

fn component(name: &str, tag: &str) -> PlannedComponent {
    PlannedComponent {
        id: NodeId(1),
        name: name.to_string(),
        tag: Some(tag.to_string()),
        class_name: Some("Generated".to_string()),
        file: Some(format!("components/{tag}.ts")),
        status: Status::Ok,
        issues: vec![],
        states: vec![StateRecipe { id: NodeId(1), props: BTreeMap::new(), attrs: BTreeMap::new(), pseudo: None }],
    }
}

fn plan_of(components: Vec<PlannedComponent>) -> ExportPlan {
    ExportPlan { components, files: vec![], notes: vec![] }
}

#[test]
fn the_alternative_name_stands_in_when_the_main_name_is_absent() {
    let composed = compose(Template::Finance, &plan_of(vec![component("Dropdown", "ui-dropdown")]));
    assert!(composed.html.contains("<ui-dropdown"), "{}", composed.html);
    assert!(!composed.missing.contains(&"Select".to_string()), "{:?}", composed.missing);
    assert!(!composed.html.contains("data-uigen-missing=\"Select\""));
}

#[test]
fn the_main_name_wins_over_an_alternative_when_both_exist() {
    let both = plan_of(vec![component("Dropdown", "ui-dropdown"), component("Select", "ui-select")]);
    let composed = compose(Template::Finance, &both);
    assert!(composed.html.contains("<ui-select"), "{}", composed.html);
    assert!(!composed.html.contains("<ui-dropdown"), "{}", composed.html);
}

#[test]
fn without_the_main_name_or_an_alternative_the_marker_carries_the_main_name() {
    let composed = compose(Template::Finance, &plan_of(vec![component("Button", "ui-button")]));
    assert!(composed.missing.contains(&"Select".to_string()), "{:?}", composed.missing);
    assert!(composed.html.contains("data-uigen-missing=\"Select\""), "{}", composed.html);
    assert!(!composed.html.contains("Dropdown"), "{}", composed.html);
}
