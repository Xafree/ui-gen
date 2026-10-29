use super::css::{DEFAULT_FONT_SIZE, Style, font_size_px, get, is_contents, is_hidden, keyword, length, number_or_percent, split_outside, words};

fn style(pairs: &[(&str, &str)]) -> Style {
    pairs.iter().map(|(key, value)| ((*key).to_owned(), (*value).to_owned())).collect()
}

// covers: T4
#[test]
fn lengths_are_read_in_px() {
    assert_eq!(length("12px", 16.0, None), Some(12.0));
    assert_eq!(length("-1.5px", 16.0, None), Some(-1.5));
    assert_eq!(length("0", 16.0, None), Some(0.0));
    assert_eq!(length("2em", 10.0, None), Some(20.0));
    assert_eq!(length("50%", 16.0, Some(40.0)), Some(20.0));
    assert_eq!(length("3.35544e+07px", 16.0, None), Some(33_554_400.0));
}

// covers: T9
#[test]
fn lengths_that_cannot_be_resolved_are_refused() {
    let refused = ["", "px", "12", "12pt", "12rem", "50%", "1 px", "abc", "nanpx", "infpx", "1e999px", "--1px", "10px 20px", "1e3"];
    for value in refused {
        assert_eq!(length(value, 16.0, None), None, "{value}");
    }
}

// covers: T6
#[test]
fn numbers_and_percentages_must_be_finite() {
    assert_eq!(number_or_percent("0.5"), Some(0.5));
    assert_eq!(number_or_percent("50%"), Some(0.5));
    for value in ["", "abc", "nan", "inf", "inf%", "%"] {
        assert_eq!(number_or_percent(value), None, "{value}");
    }
}

// covers: T5
#[test]
fn lists_split_outside_parentheses_and_quotes_only() {
    assert_eq!(words("a  b\tc"), ["a", "b", "c"]);
    assert_eq!(words("rgba(0, 0, 0, 0.2) 0px 2px"), ["rgba(0, 0, 0, 0.2)", "0px", "2px"]);
    assert_eq!(split_outside("\"A, B\", 'C', D", |c| c == ','), ["\"A, B\"", "'C'", "D"]);
    assert_eq!(split_outside("a,(b,c", |c| c == ','), ["a", "(b,c"]);
    assert_eq!(split_outside("a),b", |c| c == ','), ["a)", "b"]);
    assert_eq!(split_outside("é, ü ,", |c| c == ','), ["é", "ü"]);
    assert!(split_outside(",,", |c| c == ',').is_empty());
    assert!(words("   ").is_empty());
}

// covers: T6
#[test]
fn style_lookups_trim_and_ignore_blank_values() {
    let style = style(&[("a", "  x "), ("b", "  "), ("c", "HIDDEN")]);
    assert_eq!(get(&style, "a"), Some("x"));
    assert_eq!(get(&style, "b"), None);
    assert_eq!(get(&style, "missing"), None);
    assert_eq!(keyword(&style, "c").as_deref(), Some("hidden"));
}

// covers: T6
#[test]
fn hidden_elements_are_those_without_a_box() {
    assert!(is_hidden(&style(&[("display", "none")])));
    assert!(is_hidden(&style(&[("display", "NONE")])));
    assert!(is_hidden(&style(&[("visibility", "hidden")])));
    assert!(is_hidden(&style(&[("visibility", "collapse")])));
    assert!(!is_hidden(&style(&[("display", "flex"), ("visibility", "visible")])));
    assert!(!is_hidden(&style(&[])));
    assert!(is_contents(&style(&[("display", "contents")])));
    assert!(!is_contents(&style(&[("display", "block")])));
}

// covers: T4
#[test]
fn em_follows_the_element_font_size_or_the_browser_default() {
    assert_eq!(font_size_px(&style(&[("fontSize", "14px")])), 14.0);
    assert_eq!(font_size_px(&style(&[("fontSize", "2em")])), 2.0 * DEFAULT_FONT_SIZE);
    for fallback in [style(&[]), style(&[("fontSize", "0px")]), style(&[("fontSize", "huge")])] {
        assert_eq!(font_size_px(&fallback), DEFAULT_FONT_SIZE);
    }
}
