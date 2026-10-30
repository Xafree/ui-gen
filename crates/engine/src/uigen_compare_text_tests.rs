//! Geometric verdict on text (E5-2.5), the rule of `sidecar/parity-text.mjs`: same family (case,
//! punctuation and style suffixes ignored), size and weight, position and measured width within one
//! pixel, family installed, layer measured; pairs by order.

use crate::uigen_compare::{GEOMETRY_TOLERANCE_PX, LibText, RefText, Verdict, text_verdict};

fn reference(family: &str, size: f64, weight: u16, x: f64, y: f64, width: f64) -> RefText {
    RefText { box_: [x, y, width, 18.0], line: None, family: family.to_owned(), size, weight }
}

fn library(family: &str, size: f64, weight: u16, x: f64, y: f64, width: f64) -> LibText {
    LibText { family: family.to_owned(), size, weight, x, y, width, installed: true, width_measured: true }
}

fn roboto_ref() -> RefText {
    reference("Roboto", 16.0, 400, 10.0, 20.0, 50.0)
}

fn roboto_lib(dx: f64, dy: f64, dwidth: f64) -> LibText {
    library("Roboto", 16.0, 400, 10.0 + dx, 20.0 + dy, 50.0 + dwidth)
}

fn is_ok(verdict: &Verdict) -> bool {
    matches!(verdict, Verdict::Ok)
}

// covers: E5-2.5
#[test]
fn same_family_size_weight_and_geometry_within_one_pixel_is_ok() {
    assert!(near(GEOMETRY_TOLERANCE_PX, 1.0));
    let report = text_verdict(&[roboto_ref()], &[roboto_lib(1.0, -1.0, 1.0)]);
    assert!(is_ok(&report.verdict), "reason: {}", report.reason);
    assert!(near(report.max_dx.abs(), 1.0));
    assert!(near(report.max_dy.abs(), 1.0));
    assert!(near(report.max_dwidth.abs(), 1.0));
}

// covers: E5-2.5
#[test]
fn a_shift_of_one_and_a_half_pixels_is_ko_and_reports_the_worst_dx() {
    let references = [roboto_ref(), reference("Roboto", 16.0, 400, 10.0, 50.0, 50.0)];
    let libraries = [roboto_lib(0.5, 0.0, 0.0), library("Roboto", 16.0, 400, 11.5, 50.0, 50.0)];
    let report = text_verdict(&references, &libraries);
    assert!(!is_ok(&report.verdict));
    assert!(near(report.max_dx.abs(), 1.5));
}

// covers: E5-2.5
#[test]
fn vertical_and_width_gaps_beyond_one_pixel_are_ko() {
    let vertical = text_verdict(&[roboto_ref()], &[roboto_lib(0.0, 2.0, 0.0)]);
    assert!(!is_ok(&vertical.verdict));
    assert!(near(vertical.max_dy.abs(), 2.0));

    let wider = text_verdict(&[roboto_ref()], &[roboto_lib(0.0, 0.0, 1.5)]);
    assert!(!is_ok(&wider.verdict));
    assert!(near(wider.max_dwidth.abs(), 1.5));
}

// covers: E5-2.5
#[test]
fn the_line_box_wins_over_the_box_for_the_position() {
    let mut text = roboto_ref();
    text.line = Some([12.0, 20.0, 50.0, 18.0]);
    let report = text_verdict(&[text], &[library("Roboto", 16.0, 400, 12.0, 20.0, 50.0)]);
    assert!(is_ok(&report.verdict), "reason: {}", report.reason);
}

// covers: E5-2.5
#[test]
fn a_family_that_is_not_installed_is_ko_and_named() {
    let mut text = roboto_lib(0.0, 0.0, 0.0);
    text.installed = false;
    let report = text_verdict(&[roboto_ref()], &[text]);
    assert!(!is_ok(&report.verdict));
    assert_eq!(report.reason, "police absente : Roboto");
}

// covers: E5-2.5
#[test]
fn an_unmeasured_layer_is_ko() {
    let mut text = roboto_lib(0.0, 0.0, 0.0);
    text.width_measured = false;
    let report = text_verdict(&[roboto_ref()], &[text]);
    assert!(!is_ok(&report.verdict));
    assert!(!report.reason.is_empty());
}

// covers: E5-2.5
#[test]
fn different_counts_are_ko_with_both_numbers() {
    let more_captured = text_verdict(&[roboto_ref(), roboto_ref()], &[roboto_lib(0.0, 0.0, 0.0)]);
    assert!(!is_ok(&more_captured.verdict));
    assert!(more_captured.reason.starts_with("2 textes capturés, 1 calque(s) texte"), "reason: {}", more_captured.reason);

    let more_layers = text_verdict(&[], &[roboto_lib(0.0, 0.0, 0.0)]);
    assert!(!is_ok(&more_layers.verdict));
    assert!(more_layers.reason.starts_with("0 textes capturés, 1 calque(s) texte"), "reason: {}", more_layers.reason);
}

// covers: E5-2.5
#[test]
fn no_text_and_no_layer_is_ok() {
    let report = text_verdict(&[], &[]);
    assert!(is_ok(&report.verdict), "reason: {}", report.reason);
}

// covers: E5-2.5
#[test]
fn family_matching_ignores_case_punctuation_and_style_suffixes() {
    let cases = [
        ("Roboto", 400, "Roboto", 400, true),
        ("Roboto Medium", 500, "Roboto", 500, true),
        ("Open Sans", 400, "open-sans", 400, true),
        ("Roboto", 400, "Arial", 400, false),
        ("Roboto Condensed", 400, "Roboto", 400, false),
    ];
    for (reference_family, reference_weight, library_family, library_weight, expected_ok) in cases {
        let references = [reference(reference_family, 16.0, reference_weight, 10.0, 20.0, 50.0)];
        let libraries = [library(library_family, 16.0, library_weight, 10.0, 20.0, 50.0)];
        let report = text_verdict(&references, &libraries);
        assert_eq!(is_ok(&report.verdict), expected_ok, "{reference_family} vs {library_family}: {}", report.reason);
    }
}

// covers: E5-2.5
#[test]
fn a_different_size_or_weight_is_ko() {
    let smaller = text_verdict(&[roboto_ref()], &[library("Roboto", 14.0, 400, 10.0, 20.0, 50.0)]);
    assert!(!is_ok(&smaller.verdict));

    let bolder = text_verdict(&[roboto_ref()], &[library("Roboto", 16.0, 700, 10.0, 20.0, 50.0)]);
    assert!(!is_ok(&bolder.verdict));

    let medium_against_regular = text_verdict(
        &[reference("Roboto Medium", 16.0, 500, 10.0, 20.0, 50.0)],
        &[library("Roboto", 16.0, 400, 10.0, 20.0, 50.0)],
    );
    assert!(!is_ok(&medium_against_regular.verdict));
}

fn near(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-9
}
