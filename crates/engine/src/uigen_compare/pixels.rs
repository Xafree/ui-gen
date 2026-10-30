use super::{COLOR_THRESHOLD, PixelReport, Raster, TextBox};
use std::ops::RangeInclusive;

const MAX_YIQ_DELTA: f64 = 35215.0;
const DIMMING_ALPHA: f64 = 0.1;
const DIFFERENT: [u8; 4] = [255, 0, 0, 255];
const GOLDEN_RATIO: f64 = std::f64::consts::GOLDEN_RATIO;
const SQUARED_GOLDEN_RATIO: f64 = GOLDEN_RATIO + 1.0;

#[derive(Clone, Copy)]
struct View<'a> {
    rgba: &'a [u8],
    stride: usize,
    width: usize,
    height: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Similar,
    Antialiased,
    Different,
}

struct Overlap {
    width: usize,
    height: usize,
    union_pixels: u64,
    size_mismatch: bool,
}

struct Neighbourhood {
    columns: RangeInclusive<usize>,
    rows: RangeInclusive<usize>,
    on_edge: bool,
}

impl<'a> View<'a> {
    fn cropped(raster: &'a Raster, overlap: &Overlap) -> View<'a> {
        View { rgba: &raster.rgba, stride: raster.width as usize, width: overlap.width, height: overlap.height }
    }

    fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let at = (y * self.stride + x) * 4;
        self.rgba.get(at..at + 4).and_then(|bytes| <[u8; 4]>::try_from(bytes).ok()).unwrap_or_default()
    }

    fn cropped_offset(&self, x: usize, y: usize) -> usize {
        (y * self.width + x) * 4
    }

    fn neighbourhood(&self, x: usize, y: usize) -> Neighbourhood {
        let columns = x.saturating_sub(1)..=(x + 1).min(self.width.saturating_sub(1));
        let rows = y.saturating_sub(1)..=(y + 1).min(self.height.saturating_sub(1));
        let on_edge = x == *columns.start() || x == *columns.end() || y == *rows.start() || y == *rows.end();
        Neighbourhood { columns, rows, on_edge }
    }
}

impl Overlap {
    fn of(original: &Raster, library: &Raster) -> Option<Overlap> {
        if !original.is_well_formed() || !library.is_well_formed() {
            return None;
        }
        Some(Overlap {
            width: original.width.min(library.width) as usize,
            height: original.height.min(library.height) as usize,
            union_pixels: u64::from(original.width.max(library.width)) * u64::from(original.height.max(library.height)),
            size_mismatch: original.width != library.width || original.height != library.height,
        })
    }

    fn pixels(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

fn percent(count: u64, total: u64) -> f64 {
    if total > 0 { count as f64 / total as f64 * 100.0 } else { 0.0 }
}

fn unmeasurable() -> PixelReport {
    PixelReport {
        differing_shapes: 0,
        shapes_total: 0,
        text_differing: 0,
        text_pixels: 0,
        size_mismatch: true,
        shapes_percent: 100.0,
        text_percent: 0.0,
    }
}

fn clamp_index(value: f64, limit: usize) -> usize {
    value.clamp(0.0, limit as f64) as usize
}

fn mark_box(mask: &mut [bool], width: usize, height: usize, rect: [f64; 4]) {
    if !rect.iter().all(|value| value.is_finite()) {
        return;
    }
    let [x, y, box_width, box_height] = rect;
    let left = clamp_index(x.floor(), width);
    let top = clamp_index(y.floor(), height);
    let right = clamp_index((x + box_width).ceil(), width).max(left);
    let bottom = clamp_index((y + box_height).ceil(), height);
    for row in top..bottom {
        if let Some(cells) = mask.get_mut(row * width + left..row * width + right) {
            cells.fill(true);
        }
    }
}

fn text_mask(boxes: &[TextBox], width: usize, height: usize) -> Vec<bool> {
    let mut mask = vec![false; width * height];
    for text in boxes {
        for rect in [Some(text.box_), text.line].into_iter().flatten() {
            mark_box(&mut mask, width, height, rect);
        }
    }
    mask
}

fn background(offset: usize) -> [f64; 3] {
    let at = offset as f64;
    let channel = |divisor: f64| 48.0 + 159.0 * ((at / divisor).floor() % 2.0);
    [48.0 + 159.0 * (at % 2.0), channel(GOLDEN_RATIO), channel(SQUARED_GOLDEN_RATIO)]
}

fn channel_deltas(first: [u8; 4], second: [u8; 4], offset: usize) -> [f64; 3] {
    let [r1, g1, b1, a1] = first.map(f64::from);
    let [r2, g2, b2, a2] = second.map(f64::from);
    if first[3] == 255 && second[3] == 255 {
        return [r1 - r2, g1 - g2, b1 - b2];
    }
    let [red, green, blue] = background(offset);
    let alpha_gap = a1 - a2;
    [(r1 * a1 - r2 * a2 - red * alpha_gap) / 255.0, (g1 * a1 - g2 * a2 - green * alpha_gap) / 255.0, (b1 * a1 - b2 * a2 - blue * alpha_gap) / 255.0]
}

fn luma([r, g, b]: [f64; 3]) -> f64 {
    r * 0.298_895_31 + g * 0.586_622_47 + b * 0.114_482_23
}

fn color_delta(first: [u8; 4], second: [u8; 4], offset: usize) -> f64 {
    let deltas = channel_deltas(first, second, offset);
    let [r, g, b] = deltas;
    let y = luma(deltas);
    let i = r * 0.595_977_99 - g * 0.274_176_10 - b * 0.321_801_89;
    let q = r * 0.211_470_17 - g * 0.522_617_11 + b * 0.311_146_94;
    let delta = 0.5053 * y * y + 0.299 * i * i + 0.1957 * q * q;
    if y > 0.0 { -delta } else { delta }
}

fn brightness_delta(center: [u8; 4], neighbour: [u8; 4], offset: usize) -> f64 {
    if center == neighbour { 0.0 } else { luma(channel_deltas(center, neighbour, offset)) }
}

fn has_many_siblings(image: View, x: usize, y: usize) -> bool {
    let around = image.neighbourhood(x, y);
    let value = image.pixel(x, y);
    let mut equal = usize::from(around.on_edge);
    for column in around.columns {
        for row in around.rows.clone() {
            if (column, row) != (x, y) && image.pixel(column, row) == value {
                equal += 1;
                if equal > 2 {
                    return true;
                }
            }
        }
    }
    false
}

fn is_antialiased(image: View, other: View, x: usize, y: usize) -> bool {
    let around = image.neighbourhood(x, y);
    let center = image.pixel(x, y);
    let offset = image.cropped_offset(x, y);
    let mut equal = usize::from(around.on_edge);
    let mut darkest = (0.0, 0, 0);
    let mut brightest = (0.0, 0, 0);
    for column in around.columns {
        for row in around.rows.clone() {
            if (column, row) == (x, y) {
                continue;
            }
            let delta = brightness_delta(center, image.pixel(column, row), offset);
            if delta == 0.0 {
                equal += 1;
                if equal > 2 {
                    return false;
                }
            } else if delta < darkest.0 {
                darkest = (delta, column, row);
            } else if delta > brightest.0 {
                brightest = (delta, column, row);
            }
        }
    }
    if darkest.0 == 0.0 || brightest.0 == 0.0 {
        return false;
    }
    [darkest, brightest].into_iter().any(|(_, column, row)| has_many_siblings(image, column, row) && has_many_siblings(other, column, row))
}

fn classify(original: View, library: View, x: usize, y: usize) -> Class {
    let (first, second) = (original.pixel(x, y), library.pixel(x, y));
    if first == second {
        return Class::Similar;
    }
    let max_delta = MAX_YIQ_DELTA * COLOR_THRESHOLD * COLOR_THRESHOLD;
    if color_delta(first, second, original.cropped_offset(x, y)).abs() <= max_delta {
        return Class::Similar;
    }
    if is_antialiased(original, library, x, y) || is_antialiased(library, original, x, y) { Class::Antialiased } else { Class::Different }
}

pub fn pixel_diff(original: &Raster, library: &Raster, text: &[TextBox]) -> PixelReport {
    let Some(overlap) = Overlap::of(original, library) else {
        return unmeasurable();
    };
    let (first, second) = (View::cropped(original, &overlap), View::cropped(library, &overlap));
    let mask = text_mask(text, overlap.width, overlap.height);
    let (mut shapes, mut texts) = (0_u64, 0_u64);
    for y in 0..overlap.height {
        for x in 0..overlap.width {
            if classify(first, second, x, y) == Class::Different {
                if mask.get(y * overlap.width + x) == Some(&true) {
                    texts += 1;
                } else {
                    shapes += 1;
                }
            }
        }
    }
    let text_pixels = mask.iter().filter(|covered| **covered).count() as u64;
    let differing_shapes = shapes + overlap.union_pixels - overlap.pixels();
    let shapes_total = overlap.union_pixels - text_pixels;
    PixelReport {
        differing_shapes,
        shapes_total,
        text_differing: texts,
        text_pixels,
        size_mismatch: overlap.size_mismatch,
        shapes_percent: percent(differing_shapes, shapes_total),
        text_percent: percent(texts, text_pixels),
    }
}

fn dimmed(pixel: [u8; 4]) -> [u8; 4] {
    let gray = luma([pixel[0], pixel[1], pixel[2]].map(f64::from));
    let value = 255.0 + (gray - 255.0) * DIMMING_ALPHA * f64::from(pixel[3]) / 255.0;
    let level = value.round().clamp(0.0, 255.0) as u8;
    [level, level, level, 255]
}

fn placeholder() -> Raster {
    Raster { width: 1, height: 1, rgba: vec![0; 4] }
}

pub fn difference_image(original: &Raster, library: &Raster) -> Raster {
    let Some(overlap) = Overlap::of(original, library) else {
        return placeholder();
    };
    let width = original.width.max(library.width);
    let height = original.height.max(library.height);
    let mut rgba = Vec::new();
    let reserved = super::byte_len(width, height).is_some_and(|length| rgba.try_reserve_exact(length).is_ok());
    if !reserved {
        return placeholder();
    }
    let (first, second) = (View::cropped(original, &overlap), View::cropped(library, &overlap));
    for y in 0..height as usize {
        for x in 0..width as usize {
            let inside = x < overlap.width && y < overlap.height;
            let pixel = match inside.then(|| classify(first, second, x, y)) {
                Some(Class::Similar | Class::Antialiased) => dimmed(first.pixel(x, y)),
                Some(Class::Different) | None => DIFFERENT,
            };
            rgba.extend_from_slice(&pixel);
        }
    }
    Raster { width, height, rgba }
}
