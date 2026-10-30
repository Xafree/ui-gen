//! Pixel measure of the "Compare with the original" tab (E5-2.1 to E5-2.4, E5-2.6, E5-2.7): the lot 0
//! rule of `sidecar/parity.mjs` (YIQ threshold 0.1, antialiased pixels skipped, text boxes apart,
//! `< 1 %` of the union minus the text boxes = ok) on synthetic rasters.
//!
//! Reference values of `lot0_equivalence_*` come from running the sidecar's own `pixelmatch`
//! (`includeAA: false`, `threshold: 0.1`) and the `textMask` / `splitDifferences` logic of
//! `parity.mjs` under node on the same synthetic pairs: flat 150 shapes pixels (1.5 %, ko); edge 0
//! differing (ok; 88 with `includeAA: true`); text 95 shapes + 1200 text pixels (1.0795 %, ko; 12.95 %
//! ko without the mask).

use crate::uigen_compare::{
    COLOR_THRESHOLD, GEOMETRY_TOLERANCE_PX, PixelReport, Raster, SHAPES_MAX_PERCENT, TextBox, difference_image,
    pixel_diff,
};

const WHITE: [u8; 3] = [255, 255, 255];
const BLACK: [u8; 3] = [0, 0, 0];
const RED: [u8; 4] = [255, 0, 0, 255];

// covers: E5-2.4
#[test]
fn a_raster_without_pixels_or_of_the_wrong_length_is_refused() {
    assert!(Raster::new(2, 1, vec![0; 8]).is_ok());
    assert!(Raster::new(0, 1, Vec::new()).is_err());
    assert!(Raster::new(1, 0, Vec::new()).is_err());
    assert!(Raster::new(2, 1, vec![0; 7]).is_err());
    assert!(Raster::new(u32::MAX, u32::MAX, vec![0; 4]).is_err());
}

// covers: E5-2.4
#[test]
fn a_malformed_raster_does_not_panic_the_measure() {
    let good = solid(2, 2, WHITE);
    let malformed = [
        Raster { width: 2, height: 2, rgba: vec![0; 3] },
        Raster { width: 0, height: 0, rgba: Vec::new() },
        Raster { width: u32::MAX, height: u32::MAX, rgba: vec![0; 4] },
    ];
    for bad in &malformed {
        assert!(pixel_diff(bad, &good, &[]).size_mismatch);
        assert!(pixel_diff(&good, bad, &[]).size_mismatch);
        let _ = difference_image(bad, &good);
    }
}

fn solid(width: usize, height: usize, rgb: [u8; 3]) -> Raster {
    let rgba = [rgb[0], rgb[1], rgb[2], 255].repeat(width * height);
    Raster { width: u32::try_from(width).unwrap(), height: u32::try_from(height).unwrap(), rgba }
}

fn paint(raster: &mut Raster, x: usize, y: usize, width: usize, height: usize, rgb: [u8; 3]) {
    let stride = raster.width as usize;
    for row in y..y + height {
        for column in x..x + width {
            let at = (row * stride + column) * 4;
            raster.rgba[at..at + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
}

fn pixel(raster: &Raster, x: usize, y: usize) -> [u8; 4] {
    let at = (y * raster.width as usize + x) * 4;
    [raster.rgba[at], raster.rgba[at + 1], raster.rgba[at + 2], raster.rgba[at + 3]]
}

fn whole_box(x: f64, y: f64, width: f64, height: f64) -> TextBox {
    TextBox { box_: [x, y, width, height], line: None }
}

fn near(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-9
}

fn is_ok(report: &PixelReport) -> bool {
    report.shapes_percent < SHAPES_MAX_PERCENT
}

fn circle(samples: i64, inner: u8) -> Raster {
    let mut raster = solid(64, 64, WHITE);
    for py in 0..64_i64 {
        for px in 0..64_i64 {
            let mut covered = 0_i64;
            for j in 0..samples {
                for i in 0..samples {
                    let sx = 2 * samples * px + 2 * i + 1 - 64 * samples;
                    let sy = 2 * samples * py + 2 * j + 1 - 64 * samples;
                    if sx * sx + sy * sy <= (40 * samples) * (40 * samples) {
                        covered += 1;
                    }
                }
            }
            let value = if covered == samples * samples {
                inner
            } else {
                u8::try_from(255 - (255 * covered) / (samples * samples)).unwrap()
            };
            paint(&mut raster, usize::try_from(px).unwrap(), usize::try_from(py).unwrap(), 1, 1, [value; 3]);
        }
    }
    raster
}

fn flat_pair() -> (Raster, Raster) {
    let original = solid(100, 100, WHITE);
    let mut library = solid(100, 100, WHITE);
    paint(&mut library, 20, 20, 15, 10, BLACK);
    (original, library)
}

fn text_pair() -> (Raster, Raster, Vec<TextBox>) {
    let mut original = solid(100, 100, WHITE);
    let mut library = solid(100, 100, WHITE);
    paint(&mut original, 10, 10, 60, 20, [50, 50, 50]);
    paint(&mut library, 60, 60, 19, 5, BLACK);
    (original, library, vec![whole_box(10.0, 10.0, 60.0, 20.0)])
}

#[test]
fn thresholds_are_the_lot_zero_values() {
    assert!(near(SHAPES_MAX_PERCENT, 1.0));
    assert!(near(COLOR_THRESHOLD, 0.1));
    assert!(near(GEOMETRY_TOLERANCE_PX, 1.0));
}

// covers: E5-2.1
#[test]
fn identical_images_have_no_differing_pixel() {
    let report = pixel_diff(&solid(100, 100, WHITE), &solid(100, 100, WHITE), &[]);
    assert_eq!(report.differing_shapes, 0);
    assert!(near(report.shapes_percent, 0.0));
    assert_eq!(report.shapes_total, 10_000);
    assert!(!report.size_mismatch);
    assert!(is_ok(&report));
}

// covers: E5-2.1
#[test]
fn one_pixel_in_ten_thousand_stays_ok() {
    let original = solid(100, 100, WHITE);
    let mut library = solid(100, 100, WHITE);
    paint(&mut library, 50, 50, 1, 1, BLACK);
    let report = pixel_diff(&original, &library, &[]);
    assert_eq!(report.differing_shapes, 1);
    assert!(near(report.shapes_percent, 0.01));
    assert!(is_ok(&report));
}

// covers: E5-2.1
#[test]
fn two_percent_of_changed_pixels_is_ko() {
    let original = solid(100, 100, WHITE);
    let mut library = solid(100, 100, WHITE);
    paint(&mut library, 40, 40, 20, 10, BLACK);
    let report = pixel_diff(&original, &library, &[]);
    assert_eq!(report.differing_shapes, 200);
    assert!(near(report.shapes_percent, 2.0));
    assert!(!is_ok(&report));
}

// covers: E5-2.2
#[test]
fn pixels_in_a_text_box_count_as_text_never_as_shapes() {
    let original = solid(100, 100, WHITE);
    let mut library = solid(100, 100, WHITE);
    paint(&mut library, 20, 20, 20, 20, BLACK);
    let report = pixel_diff(&original, &library, &[whole_box(20.0, 20.0, 20.0, 20.0)]);
    assert_eq!(report.text_differing, 400);
    assert_eq!(report.text_pixels, 400);
    assert!(near(report.text_percent, 100.0));
    assert_eq!(report.differing_shapes, 0);
    assert_eq!(report.shapes_total, 10_000 - 400);
}

// covers: E5-2.2
#[test]
fn a_patch_half_inside_the_box_splits_between_text_and_shapes() {
    let original = solid(100, 100, WHITE);
    let mut library = solid(100, 100, WHITE);
    paint(&mut library, 20, 20, 20, 20, BLACK);
    let report = pixel_diff(&original, &library, &[whole_box(20.0, 20.0, 10.0, 20.0)]);
    assert_eq!(report.text_pixels, 200);
    assert_eq!(report.text_differing, 200);
    assert_eq!(report.differing_shapes, 200);
    assert_eq!(report.shapes_total, 10_000 - 200);
    assert!(near(report.text_percent, 100.0));
}

// covers: E5-2.2
#[test]
fn box_and_line_box_are_united_and_clipped_to_the_image() {
    let original = solid(100, 100, WHITE);
    let mut library = solid(100, 100, WHITE);
    paint(&mut library, 20, 20, 20, 20, BLACK);
    let united = TextBox { box_: [20.0, 20.0, 10.0, 20.0], line: Some([25.0, 20.0, 15.0, 20.0]) };
    let report = pixel_diff(&original, &library, &[united]);
    assert_eq!(report.text_pixels, 400);
    assert_eq!(report.text_differing, 400);
    assert_eq!(report.differing_shapes, 0);

    let mut corner = solid(100, 100, WHITE);
    paint(&mut corner, 90, 90, 10, 10, BLACK);
    let clipped = pixel_diff(&original, &corner, &[whole_box(90.0, 90.0, 50.0, 50.0)]);
    assert_eq!(clipped.text_pixels, 100);
    assert_eq!(clipped.text_differing, 100);
    assert_eq!(clipped.shapes_total, 10_000 - 100);
}

// covers: E5-2.2
#[test]
fn non_finite_or_off_image_boxes_are_ignored() {
    let original = solid(100, 100, WHITE);
    let mut library = solid(100, 100, WHITE);
    paint(&mut library, 20, 20, 20, 20, BLACK);
    let boxes = [
        whole_box(f64::NAN, 20.0, 20.0, 20.0),
        whole_box(20.0, f64::INFINITY, 20.0, 20.0),
        whole_box(20.0, 20.0, f64::NEG_INFINITY, 20.0),
        whole_box(-30.0, -30.0, 10.0, 10.0),
    ];
    let report = pixel_diff(&original, &library, &boxes);
    assert_eq!(report.text_pixels, 0);
    assert_eq!(report.text_differing, 0);
    assert_eq!(report.differing_shapes, 400);
    assert_eq!(report.shapes_total, 10_000);
}

// covers: E5-2.2
#[test]
fn an_image_without_a_text_box_has_no_text_pixels() {
    let (original, library) = flat_pair();
    let report = pixel_diff(&original, &library, &[]);
    assert_eq!(report.text_pixels, 0);
    assert_eq!(report.text_differing, 0);
    assert!(near(report.text_percent, 0.0));
}

// covers: E5-2.3
#[test]
fn an_antialiased_circle_edge_from_two_rasterizers_is_not_counted() {
    let hard_edged = circle(1, 0);
    let smooth = circle(4, 4);
    let report = pixel_diff(&hard_edged, &smooth, &[]);
    assert_eq!(report.differing_shapes, 0);
    assert!(is_ok(&report));
}

// covers: E5-2.3
#[test]
fn a_one_pixel_shift_of_a_solid_rectangle_is_counted() {
    let mut original = solid(64, 64, WHITE);
    let mut shifted = solid(64, 64, WHITE);
    paint(&mut original, 10, 10, 20, 20, BLACK);
    paint(&mut shifted, 11, 10, 20, 20, BLACK);
    let report = pixel_diff(&original, &shifted, &[]);
    assert_eq!(report.differing_shapes, 40);
}

// covers: E5-2.4
#[test]
fn different_sizes_compare_on_the_intersection_and_count_the_rest_as_different() {
    let original = solid(100, 100, WHITE);
    let library = solid(120, 100, WHITE);
    let report = pixel_diff(&original, &library, &[]);
    assert!(report.size_mismatch);
    assert_eq!(report.differing_shapes, 2_000);
    assert_eq!(report.shapes_total, 12_000);
    assert!(near(report.shapes_percent, 2_000.0 / 12_000.0 * 100.0));
    assert!(!is_ok(&report));
}

// covers: E5-2.4
#[test]
fn differences_inside_the_intersection_add_to_the_outside_pixels() {
    let original = solid(100, 100, WHITE);
    let mut library = solid(100, 80, WHITE);
    paint(&mut library, 10, 10, 10, 10, BLACK);
    let report = pixel_diff(&original, &library, &[]);
    assert!(report.size_mismatch);
    assert_eq!(report.differing_shapes, 100 + 2_000);
    assert_eq!(report.shapes_total, 10_000);
}

// covers: E5-2.6
#[test]
fn difference_image_paints_differing_pixels_red_and_dims_the_rest() {
    let original = solid(5, 5, [30, 30, 30]);
    let mut library = solid(5, 5, [30, 30, 30]);
    paint(&mut library, 2, 2, 1, 1, WHITE);
    let image = difference_image(&original, &library);
    assert_eq!((image.width, image.height), (5, 5));
    assert_eq!(image.rgba.len(), 5 * 5 * 4);
    assert_eq!(pixel(&image, 2, 2), RED);
    let unchanged = pixel(&image, 0, 0);
    assert_ne!(unchanged, RED);
    assert_ne!(unchanged, [30, 30, 30, 255]);
}

// covers: E5-2.6
#[test]
fn difference_image_has_the_size_of_the_union() {
    let image = difference_image(&solid(3, 2, WHITE), &solid(5, 4, WHITE));
    assert_eq!((image.width, image.height), (5, 4));
    assert_eq!(image.rgba.len(), 5 * 4 * 4);
}

// covers: E5-2.6
#[test]
fn difference_image_is_deterministic() {
    let (original, library) = flat_pair();
    let first = difference_image(&original, &library);
    let second = difference_image(&original, &library);
    assert_eq!((first.width, first.height), (second.width, second.height));
    assert_eq!(first.rgba, second.rgba);
}

// covers: E5-2.7
#[test]
fn lot0_equivalence_flat_pair_is_ko() {
    let (original, library) = flat_pair();
    let report = pixel_diff(&original, &library, &[]);
    assert_eq!(report.differing_shapes, 150);
    assert!(near(report.shapes_percent, 1.5));
    assert!(!is_ok(&report));
}

// covers: E5-2.7
#[test]
fn lot0_equivalence_edge_pair_is_ok() {
    let report = pixel_diff(&circle(1, 0), &circle(4, 4), &[]);
    assert!(is_ok(&report));
}

// covers: E5-2.7
#[test]
fn lot0_equivalence_text_pair_is_ko_with_the_box_left_out_of_the_denominator() {
    let (original, library, boxes) = text_pair();
    let report = pixel_diff(&original, &library, &boxes);
    assert_eq!(report.differing_shapes, 95);
    assert_eq!(report.text_differing, 1_200);
    assert_eq!(report.text_pixels, 1_200);
    assert_eq!(report.shapes_total, 10_000 - 1_200);
    assert!(near(report.shapes_percent, 95.0 / 8_800.0 * 100.0));
    assert!(!is_ok(&report));
}
