pub(crate) mod measure;
mod pixels;
mod text;

use serde::{Deserialize, Serialize};

use figcraft_doc::{Document, NodeId};

pub use pixels::{difference_image, pixel_diff};
pub use text::{text_verdict, weight_of_font_style};

pub fn original_raster(root: &str, png: &str) -> Result<Raster, &'static str> {
    measure::read_original(root, png)
}

/// The raster of the library `variant` that `uigen.compare` measures against the original, in the light or dark theme,
/// on the page background the original was captured on.
pub fn library_raster(doc: &Document, variant: NodeId, dark: bool) -> Result<Raster, &'static str> {
    measure::render_library(&themed(doc, variant, dark), variant, theme_of(dark))
}

fn theme_of(dark: bool) -> crate::uigen_import::Theme {
    if dark { crate::uigen_import::Theme::Dark } else { crate::uigen_import::Theme::Light }
}

pub fn themed(doc: &Document, variant: NodeId, dark: bool) -> Document {
    let mut scratch = doc.clone();
    measure::follow(&mut scratch, variant, theme_of(dark));
    scratch
}

pub const SHAPES_MAX_PERCENT: f64 = 1.0;
pub const COLOR_THRESHOLD: f64 = 0.1;
pub const GEOMETRY_TOLERANCE_PX: f64 = 1.0;

/// Whether two sizes differ by at most `GEOMETRY_TOLERANCE_PX` on each side.
pub fn sizes_agree(original: (u32, u32), library: (u32, u32)) -> bool {
    f64::from(original.0.abs_diff(library.0)) <= GEOMETRY_TOLERANCE_PX && f64::from(original.1.abs_diff(library.1)) <= GEOMETRY_TOLERANCE_PX
}

fn cropped(raster: &Raster, width: u32, height: u32) -> Raster {
    let (stride, columns) = (raster.width as usize, width as usize);
    let rgba = (0..height as usize).filter_map(|row| raster.rgba.get(row * stride * 4..(row * stride + columns) * 4)).flatten().copied().collect();
    Raster { width, height, rgba }
}

/// The two rasters on the area they share when their sizes agree within the tolerance, so that a column or a row
/// of difference in size is not counted; unchanged otherwise.
pub fn aligned(original: &Raster, library: &Raster) -> (Raster, Raster) {
    let agree =
        original.is_well_formed() && library.is_well_formed() && sizes_agree((original.width, original.height), (library.width, library.height));
    if !agree {
        return (original.clone(), library.clone());
    }
    let (width, height) = (original.width.min(library.width), original.height.min(library.height));
    (cropped(original, width, height), cropped(library, width, height))
}

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
