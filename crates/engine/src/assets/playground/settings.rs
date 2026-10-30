use figcraft_color::Color;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadiusPreset {
    Zero,
    Small,
    Medium,
    Large,
    Full,
}

impl RadiusPreset {
    pub const ALL: [RadiusPreset; 5] = [RadiusPreset::Zero, RadiusPreset::Small, RadiusPreset::Medium, RadiusPreset::Large, RadiusPreset::Full];

    /// The radius this level gives a token of `value` pixels, to the half pixel; None for the
    /// level that leaves the theme's value alone.
    pub fn scale(self, value: f64) -> Option<f64> {
        let half_pixel = |x: f64| (x * 2.0).round() / 2.0;
        match self {
            RadiusPreset::Zero => Some(0.0),
            RadiusPreset::Small => Some(half_pixel(value * 0.5)),
            RadiusPreset::Medium => None,
            RadiusPreset::Large => Some(half_pixel(value * 1.5)),
            RadiusPreset::Full => Some(9999.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RadiusShape {
    #[default]
    Round,
    Squircle,
    Bevel,
}

/// A hue (0 to 360) and how much of it the neutrals take (0 to 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neutral {
    pub hue: f64,
    pub tint: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    pub accent: Option<Color>,
    pub base: Option<Neutral>,
    pub font: Option<String>,
    pub radius: Option<RadiusPreset>,
    pub radius_shape: RadiusShape,
    pub theme: Option<String>,
    pub mode: Mode,
}
