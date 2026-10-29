use super::*;

fn rgba8(css: &str) -> [u8; 4] {
    Color::parse_css(css).unwrap_or_else(|| panic!("{css} is not read")).to_rgba8()
}

fn assert_near(css: &str, expected: [u8; 4]) {
    let actual = rgba8(css);
    assert!(actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 1), "{css}: got {actual:?}, expected {expected:?}");
}

// covers: T10
#[test]
fn percentages_are_read_against_each_channel_reference() {
    assert_near("color(srgb 100% 0% 50%)", [255, 0, 128, 255]);
    assert_near("color(srgb 1 1 1 / 50%)", [255, 255, 255, 128]);
    assert_near("oklab(100% 0 0)", [255, 255, 255, 255]);
    assert_near("oklab(0% 0% 0%)", [0, 0, 0, 255]);
    assert_eq!(rgba8("oklab(0.7 100% 0)"), rgba8("oklab(0.7 0.4 0)"));
}

// covers: T10
#[test]
fn none_reads_as_zero_and_a_missing_alpha_as_opaque() {
    assert_near("color(srgb none 1 none)", [0, 255, 0, 255]);
    assert_near("oklab(none none none)", [0, 0, 0, 255]);
    assert_near("oklab(0.5 0 0 / none)", rgba8("oklab(0.5 0 0)"));
}

// covers: T10
#[test]
fn spelling_is_forgiving_about_case_and_spaces() {
    assert_near("  COLOR( SRGB 1 0 0 / 50% )  ", [255, 0, 0, 128]);
    assert_near("OKLab(0.205 0 0/0.5)", [23, 23, 23, 128]);
}

// covers: T10
#[test]
fn srgb_linear_is_encoded_and_display_p3_grey_stays_grey() {
    assert_near("color(srgb-linear 0 0 0)", [0, 0, 0, 255]);
    assert_near("color(srgb-linear 1 1 1)", [255, 255, 255, 255]);
    assert_near("color(srgb-linear 0.5 0.5 0.5)", [188, 188, 188, 255]);
    assert_near("color(display-p3 0.5 0.5 0.5)", [128, 128, 128, 255]);
}

// covers: T10
#[test]
fn channels_beyond_the_gamut_are_clipped() {
    assert_near("color(srgb 2 -1 0.5)", [255, 0, 128, 255]);
    assert_near("color(display-p3 0 1 0)", [0, 255, 0, 255]);
}

// covers: T10
#[test]
fn malformed_and_non_finite_forms_are_refused() {
    let refused = [
        "oklab()",
        "color()",
        "color(srgb 1 0 0",
        "colour(srgb 1 0 0)",
        "color(rec2020 1 0 0)",
        "color(srgb 1e999 0 0)",
        "color(srgb nan 0 0)",
        "color(srgb 1 0 0 / inf)",
        "color(srgb 1 0 0 /)",
        "color(srgb 1 0 0 / 1 / 1)",
        "color(srgb, 1, 0, 0)",
        "color(display-p3 1 0 0 0.5)",
        "color(srgb 1 0 0 / 0.5 0.5)",
        "oklab(inf 0 0)",
        "oklab(0.5 -infinity 0)",
        "oklab(0.5 0.1 0.2 / nan)",
        "oklab(0.5 0.1 0.2 0.9)",
        "oklab(0.5, 0.1, 0.2)",
        "oklab(0.5 0.1 0.2 / 0.5 / 0.5)",
        "oklab(0.5 0.1 0.2deg)",
        "oklab(🟥 0 0)",
    ];
    for css in refused {
        assert_eq!(Color::parse_css(css), None, "{css}");
    }
}

// covers: T10
#[test]
fn huge_finite_values_come_back_in_range() {
    for css in ["oklab(1e30 1e30 -1e30)", "oklab(-1e30 0 0)", "color(srgb 1e30 -1e30 1e30)", "color(display-p3 1e30 1e30 1e30)"] {
        let c = Color::parse_css(css).unwrap_or_else(|| panic!("{css}"));
        for v in [c.r, c.g, c.b, c.a] {
            assert!(v.is_finite() && (0.0..=1.0).contains(&v), "{css}: {c:?}");
        }
    }
}

// covers: T10
#[test]
fn oklab_matches_oklch_for_the_same_colour() {
    let from_oklch = Color::parse_css("oklch(0.6 0.1 200)").unwrap();
    let (a, b) = (0.1_f32 * 200_f32.to_radians().cos(), 0.1_f32 * 200_f32.to_radians().sin());
    assert_near(&format!("oklab(0.6 {a} {b})"), from_oklch.to_rgba8());
}
