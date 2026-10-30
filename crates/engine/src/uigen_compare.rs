pub(crate) mod measure;
mod pixels;
mod text;

use serde::{Deserialize, Serialize};

pub use pixels::{difference_image, pixel_diff};
pub use text::{text_verdict, weight_of_font_style};

pub const SHAPES_MAX_PERCENT: f64 = 1.0;
pub const COLOR_THRESHOLD: f64 = 0.1;
pub const GEOMETRY_TOLERANCE_PX: f64 = 1.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterError {
    Empty,
    WrongLength,
}

fn byte_len(width: u32, height: u32) -> Option<usize> {
    usize::try_from(width).ok()?.checked_mul(usize::try_from(height).ok()?)?.checked_mul(4)
}

impl Raster {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Raster, RasterError> {
        if width == 0 || height == 0 {
            return Err(RasterError::Empty);
        }
        if byte_len(width, height) != Some(rgba.len()) {
            return Err(RasterError::WrongLength);
        }
        Ok(Raster { width, height, rgba })
    }

    fn is_well_formed(&self) -> bool {
        self.width > 0 && self.height > 0 && byte_len(self.width, self.height) == Some(self.rgba.len())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextBox {
    pub box_: [f64; 4],
    pub line: Option<[f64; 4]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PixelReport {
    pub differing_shapes: u64,
    pub shapes_total: u64,
    pub text_differing: u64,
    pub text_pixels: u64,
    pub size_mismatch: bool,
    pub shapes_percent: f64,
    pub text_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefText {
    #[serde(rename = "box")]
    pub box_: [f64; 4],
    #[serde(default)]
    pub line: Option<[f64; 4]>,
    pub family: String,
    pub size: f64,
    pub weight: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LibText {
    pub family: String,
    pub size: f64,
    pub weight: u16,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub installed: bool,
    pub width_measured: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Ok,
    Ko,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextReport {
    pub verdict: Verdict,
    pub reason: String,
    pub max_dx: f64,
    pub max_dy: f64,
    pub max_dwidth: f64,
}
