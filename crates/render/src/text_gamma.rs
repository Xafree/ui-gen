//! A browser thickens light glyphs and thins dark ones: its masks go through a curve chosen by the text's luminance.
//! Measured against Chromium on Windows, coverage to coverage, over a light and a dark page.

use figcraft_color::{Paint, PaintKind};
use tiny_skia::Pixmap;

const BLACK_TEXT_EXPONENT: f64 = 1.1;
const WHITE_TEXT_EXPONENT: f64 = 0.45;

/// The curve for text in `paints`; a gradient or layered fill has no single luminance to choose by.
pub(crate) fn exponent_of(paints: &[Paint]) -> Option<f64> {
    let [paint] = paints else { return None };
    let PaintKind::Solid { color } = &paint.kind else { return None };
    let luma = 0.2126 * f64::from(color.r) + 0.7152 * f64::from(color.g) + 0.0722 * f64::from(color.b);
    Some(BLACK_TEXT_EXPONENT + (WHITE_TEXT_EXPONENT - BLACK_TEXT_EXPONENT) * luma.clamp(0.0, 1.0))
}

/// Sends the edge pixels of premultiplied glyphs through the curve; solid pixels and empty ones stay.
pub(crate) fn lift(glyphs: &mut Pixmap, exponent: f64) {
    for pixel in glyphs.data_mut().as_chunks_mut::<4>().0 {
        let alpha = pixel[3];
        if alpha == 0 || alpha == u8::MAX {
            continue;
        }
        let coverage = f64::from(alpha) / 255.0;
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
        let white = exponent_of(&[Paint::solid(grey(1.0))]).unwrap();
        let black = exponent_of(&[Paint::solid(grey(0.0))]).unwrap();
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
        lift(&mut pixmap, 0.5);
        let data = pixmap.data();
        assert!(data[0] <= data[3] && data[2] <= data[3]);
        assert_eq!(&data[4..8], &[10, 20, 30, 255]);
    }

    #[test]
    fn only_one_solid_paint_has_a_curve() {
        let solid = Paint::solid(grey(0.5));
        assert!(exponent_of(&[]).is_none());
        assert!(exponent_of(&[solid.clone(), solid]).is_none());
    }
}
