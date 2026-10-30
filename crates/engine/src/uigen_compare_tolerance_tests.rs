//! A variant whose size is within `GEOMETRY_TOLERANCE_PX` of its original is compared on the area they
//! share; beyond it, the row is a size mismatch.

use std::io::Cursor;
use std::path::PathBuf;

use figcraft_doc::NodeId;
use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};

use crate::params::id_of;
use crate::uigen_import_support::{Bench, IMPORT, PACKAGE, Png, Scenario, button_inspect, synthetic_button, synthetic_source};
use crate::{Session, reference_of};

const FILL: [u8; 4] = [25, 118, 210, 255];
const TEXT_BOX: [u32; 4] = [10, 6, 44, 16];
const MD: (u32, u32) = (100, 36);

fn inter_button(style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = synthetic_button(style, size, state, theme);
    let ink = snapshot["root"]["children"][0]["style"]["color"].clone();
    let text_box = json!({"x": 30, "y": 26, "width": 44, "height": 16});
    snapshot["root"]["children"][0]["box"] = text_box.clone();
    snapshot["root"]["children"][0]["lineBox"] = text_box;
    snapshot["root"]["children"][0]["style"] =
        json!({"color": ink, "fontSize": "14px", "fontFamily": "Inter", "platformFont": "Inter", "fontWeight": "400"});
    snapshot
}

fn original_png(width: u32, height: u32) -> Vec<u8> {
    let mut image = RgbaImage::from_pixel(width, height, Rgba(FILL));
    let [x, y, w, h] = TEXT_BOX;
    for column in x..(x + w).min(width) {
        for row in y..(y + h).min(height) {
            image.put_pixel(column, row, Rgba([255, 255, 255, 255]));
        }
    }
    let mut bytes = Vec::new();
    image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).unwrap();
    bytes
}

fn compare_md_with_original(tag: &str, width: u32, height: u32) -> Value {
    let scenario = Scenario::new(button_inspect(true, false)).png(Png::PerVariant).snapshots(inter_button);
    let mut bench = Bench::new(tag, &[synthetic_source(false)], scenario);
    let request = json!({
        "package": PACKAGE, "component": "Button", "styles": ["solid"], "sizes": ["md"],
        "states": ["Default"], "themes": ["light"], "stage": true
    });
    bench.import(&request).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let imported = event.outcome.clone().unwrap_or_else(|error| panic!("the import failed: {}", error.message));
    let set = id_of(&imported["set"]).unwrap();
    let root = PathBuf::from(bench.session.docs[0].path.clone().unwrap()).parent().unwrap().to_path_buf();
    let variant: NodeId = bench.session.docs[0].doc.node(set).unwrap().children[0];
    let reference = reference_of(&bench.session.docs[0].doc, variant).unwrap();
    assert_eq!((reference.themes["light"].width, reference.themes["light"].height), MD);
    let png = root.join(reference.themes["light"].png.as_deref().unwrap());
    std::fs::create_dir_all(png.parent().unwrap()).unwrap();
    std::fs::write(&png, original_png(width, height)).unwrap();
    compare(&mut bench.session, set)
}

fn compare(session: &mut Session, set: NodeId) -> Value {
    session.execute("uigen.compare", &json!({"set": set})).unwrap_or_else(|error| panic!("uigen.compare: {error}"))
}

// covers: E5-3.5
#[test]
fn a_library_one_pixel_wider_than_its_original_is_compared_on_the_shared_area() {
    let result = compare_md_with_original("compare-one-pixel", MD.0 - 1, MD.1);

    let row = &result["rows"][0];
    assert_eq!(row["status"], json!("ok"), "{row}");
    assert_eq!((row["original"]["width"].as_u64(), row["library"]["width"].as_u64()), (Some(99), Some(100)), "{row}");
    assert_eq!((row["original"]["height"].as_u64(), row["library"]["height"].as_u64()), (Some(36), Some(36)), "{row}");
    assert!(row["shapes"]["percent"].as_f64().unwrap() < 1.0, "{row}");
}

// covers: E5-3.5
#[test]
fn a_library_two_pixels_wider_than_its_original_is_a_size_mismatch() {
    let result = compare_md_with_original("compare-two-pixels", MD.0 - 2, MD.1);

    let row = &result["rows"][0];
    assert_eq!(row["status"], json!("sizeMismatch"), "{row}");
    assert_eq!((row["original"]["width"].as_u64(), row["library"]["width"].as_u64()), (Some(98), Some(100)), "{row}");
}
