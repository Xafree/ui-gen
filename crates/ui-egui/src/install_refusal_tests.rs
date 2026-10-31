use figcraft_project::ProjectError;
use figcraft_project::npm::parse_install_line;

use crate::dialogs::install_refusal;

#[test]
fn every_refusal_of_an_install_line_is_worded_by_the_ui_with_the_english_text_of_the_parser() {
    let lines = [
        "",
        "npm uninstall a",
        "npm",
        "yarn add a",
        "A",
        "-a",
        "a.tgz",
        "a@^1.0.0",
        "a@1",
        &format!("{}@1.0.0", "a".repeat(215)),
        &format!("a@1.0.0-{}", "b".repeat(70)),
    ];
    for line in lines {
        let error = parse_install_line(line).expect_err(line);
        let ProjectError::Invalid { field, reason } = &error else { panic!("{line:?}: {error}") };
        let expected = if field == "line" { reason.clone() } else { format!("{field}: {reason}") };
        assert_eq!(install_refusal::text(&error), Some(expected), "{line:?}");
    }
}
