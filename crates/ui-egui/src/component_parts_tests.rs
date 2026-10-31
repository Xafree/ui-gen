use crate::component_parts::parts;

fn hidden(components: &[(&str, Option<&str>)]) -> Vec<String> {
    let mut found: Vec<String> = parts(components).into_iter().collect();
    found.sort();
    found
}

#[test]
fn a_static_part_of_a_compound_goes_when_its_parent_is_offered() {
    let list = [("Panel", None), ("PanelCaption", Some("Panel")), ("PanelRoot", Some("Panel")), ("Lone", Some("Gone"))];
    assert_eq!(hidden(&list), ["PanelCaption", "PanelRoot"]);
}

#[test]
fn without_statics_a_prefix_needs_an_exported_parent_and_a_word_for_a_part() {
    let list = [("Card", None), ("CardRoot", None), ("CardHeader", None), ("OrphanHeader", None)];
    assert_eq!(hidden(&list), ["CardHeader", "CardRoot"]);
}

#[test]
fn a_component_named_after_another_stays() {
    let list = [
        ("Button", None),
        ("ButtonGroup", None),
        ("Close", None),
        ("CloseButton", None),
        ("Number", None),
        ("NumberField", None),
        ("Alert", None),
        ("AlertDialog", None),
        ("Card", None),
        ("Cardinal", None),
    ];
    assert!(hidden(&list).is_empty(), "{:?}", hidden(&list));
}
