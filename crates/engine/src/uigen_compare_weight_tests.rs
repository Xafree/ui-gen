use crate::uigen_compare::weight_of_font_style;

#[test]
fn named_styles_map_to_their_css_weight() {
    let cases = [
        ("Regular", 400),
        ("", 400),
        ("Medium", 500),
        ("Semi Bold", 600),
        ("ExtraBold", 800),
        ("Extra-Light Italic", 200),
        ("Light", 300),
        ("Bold", 700),
        ("Black", 900),
    ];
    for (style, expected) in cases {
        assert_eq!(weight_of_font_style(style), expected, "{style}");
    }
}
