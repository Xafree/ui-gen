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
fn oklab_is_read() {
    assert_near("oklab(0.205 0 0)", [23, 23, 23, 255]);
    assert_near("oklab(0.628 0.2249 0.1258)", [255, 0, 0, 255]);
    assert_near("oklab(0.205 0 0 / 0.5)", [23, 23, 23, 128]);
}

// covers: T10
#[test]
fn color_function_reads_srgb_with_alpha() {
    assert_near("color(srgb 0.0941176 0.0941176 0.105882 / 0.9)", [24, 24, 27, 230]);
    assert_near("color(srgb 1 0 0)", [255, 0, 0, 255]);
}

// covers: T10
#[test]
fn color_function_reads_display_p3() {
    let red = Color::parse_css("color(display-p3 1 0 0)").expect("display-p3 is read");
    assert!(red.r > 0.8 && red.g < 0.4 && red.b < 0.4, "{red:?}");
    assert_eq!(red.a, 1.0);
    let white = Color::parse_css("color(display-p3 1 1 1 / 0.5)").expect("display-p3 with alpha is read");
    assert!(white.r > 0.99 && white.g > 0.99 && white.b > 0.99, "{white:?}");
    assert!((white.a - 0.5).abs() < 0.01, "{white:?}");
}

// covers: T10
#[test]
fn the_other_chromium_forms_keep_their_alpha() {
    assert_near("rgb(25, 118, 210)", [25, 118, 210, 255]);
    assert_near("rgba(0, 0, 0, 0.12)", [0, 0, 0, 31]);
    assert_near("rgb(0 0 0 / 0.12)", [0, 0, 0, 31]);
    assert_near("hsl(210 79% 46% / 0.9)", [25, 117, 210, 230]);
    assert_near("oklch(0.6 0.2 30 / 0.9)", [222, 62, 45, 230]);
}

// covers: T10
#[test]
fn broken_forms_are_refused() {
    for css in ["oklab(", "oklab(0.5 0.1)", "oklab(a b c)", "color(foo 1 2 3)", "color(srgb 1 0)", "color(srgb)", "color("] {
        assert_eq!(Color::parse_css(css), None, "{css}");
    }
}
