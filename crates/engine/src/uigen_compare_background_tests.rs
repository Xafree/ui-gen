//! A library variant that paints nothing behind itself is measured on the page background its original was captured on.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use figcraft_doc::NodeId;
use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};

use crate::params::id_of;
use crate::uigen_compare::library_raster;
use crate::uigen_compare::measure::page_background;
use crate::uigen_import::Theme;
use crate::uigen_import_support::{Bench, IMPORT, PACKAGE, Png, Scenario, button_inspect, synthetic_button, synthetic_source};
use crate::{Session, reference_of};

const TEXT_BOX: [u32; 4] = [30, 26, 44, 16];
const SIZE: (u32, u32) = (100, 36);

fn border_of(theme: &str) -> [u8; 4] {
    if theme == "dark" { [144, 202, 249, 255] } else { [25, 118, 210, 255] }
}

fn rgb(color: [u8; 4]) -> String {
    format!("rgb({}, {}, {})", color[0], color[1], color[2])
}

fn bordered(style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = synthetic_button(style, size, state, theme);
    let border = rgb(border_of(theme));
    let text_box = json!({"x": TEXT_BOX[0], "y": TEXT_BOX[1], "width": TEXT_BOX[2], "height": TEXT_BOX[3]});
    snapshot["root"]["children"][0]["box"] = text_box.clone();
    snapshot["root"]["children"][0]["lineBox"] = text_box;
    snapshot["root"]["children"][0]["style"] =
        json!({"color": border, "fontSize": "14px", "fontFamily": "Inter", "platformFont": "Inter", "fontWeight": "400"});
    snapshot["root"]["style"] = json!({"backgroundColor": "rgba(0, 0, 0, 0)"});
    for side in ["Top", "Right", "Bottom", "Left"] {
        snapshot["root"]["style"][format!("border{side}Style")] = json!("solid");
        snapshot["root"]["style"][format!("border{side}Width")] = json!("1px");
        snapshot["root"]["style"][format!("border{side}Color")] = json!(border);
    }
    snapshot
}

fn capture(theme: &str) -> Vec<u8> {
    let background = page_background(if theme == "dark" { Theme::Dark } else { Theme::Light });
    let mut image = RgbaImage::from_pixel(SIZE.0, SIZE.1, Rgba([background[0], background[1], background[2], 255]));
    for x in 0..SIZE.0 {
        for y in 0..SIZE.1 {
            if x == 0 || y == 0 || x == SIZE.0 - 1 || y == SIZE.1 - 1 {
                image.put_pixel(x, y, Rgba(border_of(theme)));
            }
        }
    }
    let mut bytes = Vec::new();
    image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).unwrap();
    bytes
}

fn imported_outline(tag: &str) -> (Bench, NodeId, PathBuf) {
    let scenario = Scenario::new(button_inspect(true, false)).png(Png::PerVariant).snapshots(bordered);
    let mut bench = Bench::new(tag, &[synthetic_source(false)], scenario);
    let request = json!({
        "package": PACKAGE, "component": "Button", "styles": ["outline"], "sizes": ["md"],
        "states": ["Default"], "themes": ["light", "dark"], "stage": true
    });
    bench.import(&request).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let imported = event.outcome.clone().unwrap_or_else(|error| panic!("the import failed: {}", error.message));
    let set = id_of(&imported["set"]).unwrap();
    let root = PathBuf::from(bench.session.docs[0].path.clone().unwrap()).parent().unwrap().to_path_buf();
    (bench, set, root)
}

fn write_captures(bench: &Bench, set: NodeId, root: &Path) {
    let variant = bench.session.docs[0].doc.node(set).unwrap().children[0];
    let reference = reference_of(&bench.session.docs[0].doc, variant).unwrap();
    for theme in ["light", "dark"] {
        let png = root.join(reference.themes[theme].png.as_deref().unwrap());
        std::fs::create_dir_all(png.parent().unwrap()).unwrap();
        std::fs::write(&png, capture(theme)).unwrap();
    }
}

fn compare(session: &mut Session, set: NodeId, theme: &str) -> Value {
    session.execute("uigen.compare", &json!({"set": set, "theme": theme})).unwrap_or_else(|error| panic!("uigen.compare: {error}"))
}

// covers: E5-3.5
#[test]
fn a_variant_with_a_border_and_no_fill_matches_its_capture_on_the_page_background() {
    for theme in ["light", "dark"] {
        let (mut bench, set, root) = imported_outline(&format!("compare-background-{theme}"));
        write_captures(&bench, set, &root);

        let result = compare(&mut bench.session, set, theme);

        let row = &result["rows"][0];
        assert_eq!(row["status"], json!("ok"), "{theme}: {row}");
        assert!(row["shapes"]["percent"].as_f64().unwrap() < 1.0, "{theme}: {row}");
    }
}

// covers: E5-3.1
#[test]
fn the_library_raster_is_opaque_and_clear_areas_take_the_page_background() {
    for (dark, theme) in [(false, Theme::Light), (true, Theme::Dark)] {
        let (bench, set, _) = imported_outline(&format!("compare-opaque-{dark}"));
        let variant = bench.session.docs[0].doc.node(set).unwrap().children[0];

        let raster = library_raster(bench.session.docs[0].doc.as_ref(), variant, dark).unwrap();

        assert!(raster.rgba.chunks(4).all(|pixel| pixel[3] == 255), "every pixel is opaque");
        let background = page_background(theme);
        let middle = ((SIZE.1 / 2 * raster.width + 4) * 4) as usize;
        assert_eq!(raster.rgba[middle..middle + 3], background, "the inside of the button is the page");
    }
}

// covers: E5-3.1
#[test]
fn the_page_backgrounds_are_those_of_the_capture_service() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sidecar/render.mjs")).unwrap();
    let line = source.lines().find(|line| line.starts_with("export const BACKGROUNDS")).expect("the capture service declares its backgrounds");
    let hex = |color: [u8; 3]| format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]);
    assert!(line.contains(&format!("light: '{}'", hex(page_background(Theme::Light)))), "{line}");
    assert!(line.contains(&format!("dark: '{}'", hex(page_background(Theme::Dark)))), "{line}");
}
