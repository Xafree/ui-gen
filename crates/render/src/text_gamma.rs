//! A browser thickens light glyphs and thins dark ones: its masks go through a curve chosen by the text's luminance.
//! Measured against Chromium on Windows, coverage to coverage, over a light and a dark page.

use figcraft_color::{Paint, PaintKind};
use tiny_skia::Pixmap;

const BLACK_TEXT_EXPONENT: f64 = 1.1;
const WHITE_TEXT_EXPONENT: f64 = 0.45;

#[derive(Clone, Copy)]
pub(crate) struct Curve {
    exponent: f64,
    /// The alpha of full ink: what a fully covered pixel holds before any curve.
    base: f64,
}

/// The curve for text in `paints`; a gradient or layered fill has no single luminance to choose by.
pub(crate) fn curve_of(paints: &[Paint]) -> Option<Curve> {
    let [paint] = paints else { return None };
    let PaintKind::Solid { color } = &paint.kind else { return None };
    let luma = 0.2126 * f64::from(color.r) + 0.7152 * f64::from(color.g) + 0.0722 * f64::from(color.b);
    let exponent = BLACK_TEXT_EXPONENT + (WHITE_TEXT_EXPONENT - BLACK_TEXT_EXPONENT) * luma.clamp(0.0, 1.0);
    Some(Curve { exponent, base: f64::from(color.a) * f64::from(paint.opacity) })
}

/// Sends the edge pixels of premultiplied glyphs through the curve; solid pixels and empty ones stay.
pub(crate) fn lift(glyphs: &mut Pixmap, Curve { exponent, base }: Curve) {
    if !(base.is_finite() && base > 0.0) {
        return;
    }
    let solid = 1.0 - 1.0 / (255.0 * base);
    for pixel in glyphs.data_mut().as_chunks_mut::<4>().0 {
        let coverage = f64::from(pixel[3]) / 255.0 / base;
        if coverage <= 0.0 || coverage >= solid {
            continue;
        }
        let ratio = coverage.powf(exponent) / coverage;
        for channel in pixel.iter_mut() {
            *channel = (f64::from(*channel) * ratio).round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use figcraft_color::Color;

    fn translucent_white_at(coverage: f64) -> u8 {
        let paint = Paint::solid(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.7 });
        let curve = curve_of(std::slice::from_ref(&paint)).unwrap();
        let alpha = (0.7 * coverage * 255.0).round() as u8;
        let mut pixmap = edge(alpha);
        lift(&mut pixmap, curve);
        pixmap.data()[3]
    }

    #[test]
    fn a_translucent_fill_is_not_read_as_coverage() {
        assert_eq!(translucent_white_at(1.0), 179, "full ink keeps the alpha of its colour");
        let half = f64::from(translucent_white_at(0.5));
        let expected = 0.7 * 0.5_f64.powf(WHITE_TEXT_EXPONENT) * 255.0;
        assert!((half - expected).abs() <= 1.0, "{half} vs {expected}");
    }

    #[test]
    fn the_opacity_of_the_paint_is_part_of_its_base() {
        let mut paint = Paint::solid(grey(1.0));
        paint.opacity = 0.5;
        let curve = curve_of(&[paint]).unwrap();
        let mut pixmap = edge(128);
        lift(&mut pixmap, curve);
        assert_eq!(pixmap.data()[3], 128);
    }

    fn grey(level: f32) -> Color {
        Color { r: level, g: level, b: level, a: 1.0 }
    }

    fn edge(alpha: u8) -> Pixmap {
        let mut pixmap = Pixmap::new(1, 1).unwrap();
        pixmap.data_mut().copy_from_slice(&[alpha, alpha, alpha, alpha]);
        pixmap
    }

    #[test]
    fn white_text_grows_its_edges_and_black_text_thins_them() {
        let white = curve_of(&[Paint::solid(grey(1.0))]).unwrap();
        let black = curve_of(&[Paint::solid(grey(0.0))]).unwrap();
        let (mut light, mut dark) = (edge(128), edge(128));
        lift(&mut light, white);
        lift(&mut dark, black);
        assert!(light.data()[3] > 180, "{}", light.data()[3]);
        assert!(dark.data()[3] < 128, "{}", dark.data()[3]);
    }

    #[test]
    fn a_premultiplied_pixel_stays_premultiplied_and_solid_ink_stays() {
        let mut pixmap = Pixmap::new(2, 1).unwrap();
        pixmap.data_mut().copy_from_slice(&[64, 0, 32, 128, 10, 20, 30, 255]);
        lift(&mut pixmap, Curve { exponent: 0.5, base: 1.0 });
        let data = pixmap.data();
        assert!(data[0] <= data[3] && data[2] <= data[3]);
        assert_eq!(&data[4..8], &[10, 20, 30, 255]);
    }

    #[test]
    fn only_one_solid_paint_has_a_curve() {
        let solid = Paint::solid(grey(0.5));
        assert!(curve_of(&[]).is_none());
        assert!(curve_of(&[solid.clone(), solid]).is_none());
    }
}
