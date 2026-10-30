use figcraft_color::{Color, Oklch};

use super::settings::{Neutral, RadiusPreset, Settings};

const MIN_ACCENT_CHROMA: f32 = 0.085;
const MAX_BASE_TINT: f64 = 0.6;
const ACCENT_TRIES: usize = 32;

/// SplitMix64: a small seeded generator, so a draw can be replayed from its seed.
struct Generator(u64);

impl Generator {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// In [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn below(&mut self, n: usize) -> usize {
        ((self.unit() * n as f64) as usize).min(n.saturating_sub(1))
    }
}

fn vivid_accent(rng: &mut Generator) -> Color {
    for _ in 0..ACCENT_TRIES {
        let (h, l, c) = (rng.unit() * 360.0, 0.55 + rng.unit() * 0.2, 0.09 + rng.unit() * 0.08);
        let color = Oklch { l: l as f32, c: c as f32, h: h as f32 }.to_color();
        if Oklch::from_color(color).c >= MIN_ACCENT_CHROMA {
            return color;
        }
    }
    Oklch { l: 0.62, c: 0.15, h: 29.0 }.to_color()
}

/// Settings drawn from `seed`: the same seed gives the same ones. The mode and the theme stay
/// with the caller.
pub fn random_settings(seed: u64, fonts: &[String], _themes: &[String]) -> Settings {
    let mut rng = Generator(seed);
    let accent = vivid_accent(&mut rng);
    let base = Neutral { hue: rng.unit() * 360.0, tint: rng.unit() * MAX_BASE_TINT };
    let font = fonts.get(rng.below(fonts.len())).cloned();
    let radius = RadiusPreset::ALL.get(rng.below(RadiusPreset::ALL.len())).copied();
    Settings { accent: Some(accent), base: Some(base), font, radius, ..Settings::default() }
}
