use super::*;

const BOX: &str = r#"{"x":0,"y":0,"width":10,"height":10}"#;
const SPIN: &str = r#"{"name":"spin","durationMs":1000,"easing":"linear","iterations":"infinite","properties":["transform"]}"#;

fn snapshot_with_element_fields(fields: &str) -> String {
    format!(
        r#"{{"version":1,"viewport":{{"width":400,"height":200,"deviceScaleFactor":1}},"root":{{"type":"element","tag":"div","box":{BOX}{fields}}}}}"#
    )
}

fn error_message(json: &str) -> String {
    match parse_snapshot(json) {
        Ok(_) => panic!("expected the snapshot to be refused"),
        Err(error) => error.to_string().to_lowercase(),
    }
}

fn root_element(json: &str) -> Element {
    match parse_snapshot(json).unwrap().root {
        DomNode::Element(element) => element,
        other => panic!("an element root was expected, got {other:?}"),
    }
}

fn vars_json(count: usize) -> String {
    let entries: Vec<String> = (0..count).map(|index| format!(r#""prop{index}":"--v{index}""#)).collect();
    format!(r#","vars":{{{}}}"#, entries.join(","))
}

fn animations_json(count: usize) -> String {
    format!(r#","animations":[{}]"#, vec![SPIN; count].join(","))
}

// covers: E3-3.6
#[test]
fn the_new_optional_fields_are_read() {
    let fields = r##","className":"MuiButton-root","vars":{"backgroundColor":"--syn-primary","borderTopLeftRadius":"--syn-radius"},"animations":[{"name":"spin","durationMs":1000,"easing":"linear","iterations":"infinite","properties":["transform"]},{"name":"fade","durationMs":250.5,"easing":"ease-in","iterations":3,"properties":["opacity","color"]}]"##;

    let element = root_element(&snapshot_with_element_fields(fields));

    assert_eq!(element.class_name.as_deref(), Some("MuiButton-root"));
    assert_eq!(element.vars.get("backgroundColor").map(String::as_str), Some("--syn-primary"));
    assert_eq!(element.vars.len(), 2);
    assert_eq!(element.animations.len(), 2);
    assert_eq!(element.animations[0].name, "spin");
    assert_eq!(element.animations[0].iterations, None, "infinite");
    assert_eq!(element.animations[0].properties, ["transform"]);
    assert_eq!(element.animations[1].duration_ms, 250.5);
    assert_eq!(element.animations[1].iterations, Some(3.0));
    assert_eq!(element.animations[1].easing, "ease-in");
}

// covers: E3-3.6
#[test]
fn a_snapshot_without_the_new_fields_reads_as_before() {
    let element = root_element(&snapshot_with_element_fields(r#","style":{"display":"flex","columnGap":"4px"}"#));

    assert!(element.class_name.is_none());
    assert!(element.vars.is_empty());
    assert!(element.animations.is_empty());
    assert_eq!(element.style.get("display").map(String::as_str), Some("flex"));
}

// covers: E3-3.6
#[test]
fn a_var_name_that_is_not_a_css_custom_property_is_refused_naming_vars() {
    let too_long = format!("--{}", "a".repeat(129));
    for name in ["primary", "-primary", "--", "--has space", "--semi;colon", "--a)b", "--é", too_long.as_str()] {
        let fields = format!(r#","vars":{{"backgroundColor":"{name}"}}"#);

        let message = error_message(&snapshot_with_element_fields(&fields));

        assert!(message.contains("vars"), "{name:?}: {message}");
    }
}

// covers: E3-3.6
#[test]
fn a_var_name_of_the_allowed_alphabet_and_length_is_accepted() {
    let longest = format!("--{}", "a".repeat(128));
    for name in ["--p", "--syn-primary_2", "--A-Z_0-9", longest.as_str()] {
        let fields = format!(r#","vars":{{"backgroundColor":"{name}"}}"#);

        assert!(parse_snapshot(&snapshot_with_element_fields(&fields)).is_ok(), "{name:?}");
    }
}

// covers: E3-3.6
#[test]
fn more_than_64_vars_are_refused_naming_vars_while_64_are_accepted() {
    assert!(parse_snapshot(&snapshot_with_element_fields(&vars_json(64))).is_ok());

    let message = error_message(&snapshot_with_element_fields(&vars_json(65)));

    assert!(message.contains("vars"), "{message}");
}

// covers: E3-3.6
#[test]
fn more_than_16_animations_are_refused_naming_animations_while_16_are_accepted() {
    assert!(parse_snapshot(&snapshot_with_element_fields(&animations_json(16))).is_ok());

    let message = error_message(&snapshot_with_element_fields(&animations_json(17)));

    assert!(message.contains("animations"), "{message}");
}

// covers: E3-3.6
#[test]
fn an_animation_duration_that_is_not_finite_is_refused() {
    let null_duration = r#","animations":[{"name":"spin","durationMs":null,"easing":"linear","iterations":"infinite","properties":[]}]"#;
    let overflowing = r#","animations":[{"name":"spin","durationMs":1e400,"easing":"linear","iterations":1,"properties":[]}]"#;

    let message = error_message(&snapshot_with_element_fields(null_duration));
    assert!(message.contains("duration"), "{message}");
    assert!(parse_snapshot(&snapshot_with_element_fields(overflowing)).is_err());
}

// covers: E3-3.6
#[test]
fn the_new_fields_on_a_nested_element_are_validated_too() {
    let nested = format!(
        r#"{{"version":1,"viewport":{{"width":400,"height":200,"deviceScaleFactor":1}},"root":{{"type":"element","tag":"div","box":{BOX},"children":[{{"type":"element","tag":"span","box":{BOX},"vars":{{"color":"nope"}}}}]}}}}"#
    );

    let message = error_message(&nested);

    assert!(message.contains("vars"), "{message}");
}
