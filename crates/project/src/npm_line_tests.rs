use crate::npm::{LineRefusal, MAX_PACKAGES, line_refusal, parse_install_line};

#[test]
fn the_line_stops_at_the_number_of_packages_the_service_takes() {
    let names = |count: usize| (0..count).map(|index| format!("p{index}")).collect::<Vec<_>>().join(" ");
    assert_eq!(parse_install_line(&names(MAX_PACKAGES)).unwrap().len(), MAX_PACKAGES);
    let error = parse_install_line(&names(MAX_PACKAGES + 1)).unwrap_err();
    assert_eq!(line_refusal(&error), Some(LineRefusal::TooMany { count: MAX_PACKAGES + 1, max: MAX_PACKAGES }));
    assert!(error.to_string().contains(&format!("{} packages on one line: {MAX_PACKAGES} at most", MAX_PACKAGES + 1)), "{error}");
}
