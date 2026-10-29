//! `export.litCompare`: how many pixels of a node's rendering differ from a PNG, the contract a
//! harness uses to judge a Lit element against the design. The PNG is read, never written.

use figcraft_doc::ExportSize;
use figcraft_render::{Pixmap, images, render_node};
use serde_json::{Value, json};

use crate::cmd::CommandSpec;
use crate::params::{f64_of, get, req_id};
use crate::{Result, bad, file_access, spec};

const CMD: &str = "export.litCompare";
const DEFAULT_TOLERANCE: f64 = 8.0;
const MAX_IMAGE_BYTES: u64 = 64 << 20;
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// How two images of the same size differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelDiff {
    pub total: u64,
    pub differing: u64,
    /// The images are not the same size: every pixel counts as differing.
    pub size_mismatch: bool,
}

impl PixelDiff {
    pub fn ratio(&self) -> f64 {
        if self.total == 0 { 0.0 } else { self.differing as f64 / self.total as f64 }
    }
}

/// The pixels of `a` that differ from those of `b` by more than `tolerance` on a channel.
pub fn pixel_diff(a: &Pixmap, b: &Pixmap, tolerance: u8) -> PixelDiff {
    let total = u64::from(a.width()) * u64::from(a.height());
    if (a.width(), a.height()) != (b.width(), b.height()) {
        return PixelDiff { total, differing: total, size_mismatch: true };
    }
    let differing =
        a.data().chunks_exact(4).zip(b.data().chunks_exact(4)).filter(|(p, q)| p.iter().zip(*q).any(|(x, y)| x.abs_diff(*y) > tolerance)).count()
            as u64;
    PixelDiff { total, differing, size_mismatch: false }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![spec!(
        "export.litCompare",
        "Compare a rendering with a PNG",
        "{id, image (path of a PNG), tolerance?: 0..255 = 8 (largest gap of a colour channel that is not a difference), scale?: 1} → {width, height, total, differing, ratio, sizeMismatch}  renders the node and counts the pixels that differ from the PNG; an image of another size is a size mismatch (every pixel differs); reads the PNG and writes nothing",
        false,
        |s, p| {
            let id = req_id(CMD, p, "id")?;
            let path = match get(p, "image") {
                Some(Value::String(path)) if !path.is_empty() => path,
                _ => return Err(bad(CMD, "`image` must be the path of a PNG")),
            };
            let tolerance = number(p, "tolerance", DEFAULT_TOLERANCE, 255.0)?;
            let scale = number(p, "scale", 1.0, ExportSize::MAX_SCALE)?;
            if tolerance.fract() != 0.0 || scale <= 0.0 {
                return Err(bad(CMD, "`tolerance` must be a whole number from 0 to 255 and `scale` above 0"));
            }
            let doc = s.doc()?;
            if doc.node(id).is_none() {
                return Err(bad(CMD, format!("{id} is not a node")));
            }
            let bytes = file_access::read_capped(path, MAX_IMAGE_BYTES).map_err(|e| bad(CMD, e.to_string()))?;
            if !bytes.starts_with(&PNG_SIGNATURE) {
                return Err(bad(CMD, format!("{path}: not a PNG")));
            }
            let image = images::decode(&bytes).ok_or_else(|| bad(CMD, format!("{path}: not a PNG that can be read")))?;
            let rendered = render_node(doc, id, scale).ok_or_else(|| bad(CMD, format!("{id} can't be rendered at scale {scale}")))?;
            let diff = pixel_diff(&rendered, &image, tolerance as u8);
            Ok(json!({
                "width": rendered.width(),
                "height": rendered.height(),
                "total": diff.total,
                "differing": diff.differing,
                "ratio": diff.ratio(),
                "sizeMismatch": diff.size_mismatch,
            }))
        }
    )]
}

fn number(p: &Value, key: &str, default: f64, max: f64) -> Result<f64> {
    match get(p, key) {
        None => Ok(default),
        Some(v) => f64_of(v).filter(|n| (0.0..=max).contains(n)).ok_or_else(|| bad(CMD, format!("`{key}` must be a number from 0 to {max}"))),
    }
}
