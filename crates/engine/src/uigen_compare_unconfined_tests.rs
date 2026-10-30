//! `uigen.compare` called as the interface calls it, with no automation roots: it still never reads an
//! image that lies outside `.uigen/captures`, whatever a hand-edited reference points at (E5-3.7).

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};

use crate::params::id_of;
use crate::uigen_import_support::{Bench, IMPORT, PACKAGE, Png, Scenario, button_inspect, synthetic_source};
use crate::{REFERENCE_KEY, Session};

const COMPARE: &str = "uigen.compare";
const CAPTURES: &str = ".uigen/captures";

fn lab(tag: &str) -> (Bench, PathBuf, Value) {
    let mut bench = Bench::new(tag, &[synthetic_source(false)], Scenario::new(button_inspect(true, false)).png(Png::PerVariant));
    let request = json!({
        "package": PACKAGE, "component": "Button", "styles": ["solid"], "sizes": ["sm", "md"],
        "states": ["Default"], "themes": ["light"], "stage": true
    });
    bench.import(&request).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let imported = event.outcome.clone().unwrap_or_else(|error| panic!("the import failed: {}", error.message));
    let placed = bench.session.execute("uigen.placeImported", &json!({"set": imported["set"]})).unwrap();
    let file = bench.session.docs[0].path.clone().unwrap();
    let root = PathBuf::from(file).parent().unwrap().to_path_buf();
    (bench, root, placed["set"].clone())
}

fn write_png(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut bytes = Vec::new();
    RgbaImage::from_pixel(100, 36, Rgba([25, 118, 210, 255])).write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn point_every_light_png_at(session: &mut Session, set: &Value, png: &str) {
    let set = id_of(set).unwrap();
    let variants = session.docs[0].doc.node(set).unwrap().children.clone();
    for variant in variants {
        let raw = session.docs[0].doc.node(variant).unwrap().plugin_data.get(REFERENCE_KEY).cloned().unwrap();
        let mut reference: Value = serde_json::from_str(&raw).unwrap();
        reference["themes"]["light"]["png"] = json!(png);
        session.execute("node.set", &json!({"id": variant, "props": {"pluginData": {REFERENCE_KEY: reference.to_string()}}})).unwrap();
    }
}

#[cfg(unix)]
fn link_dir(link: &Path, target: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn link_dir(link: &Path, target: &Path) -> bool {
    std::os::windows::fs::symlink_dir(target, link).is_ok()
        || std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(link).arg(target).output().is_ok_and(|done| done.status.success())
}

fn assert_every_row_is_missing(result: &Value) {
    let rows = result["rows"].as_array().unwrap_or_else(|| panic!("no rows in {result}"));
    assert_eq!(rows.len(), 2, "{result}");
    for row in rows {
        assert_eq!(row["status"], json!("missingReference"), "{row}");
    }
}

// covers: E5-3.7
#[test]
fn without_automation_roots_a_png_reached_through_a_link_out_of_the_captures_folder_is_never_read() {
    let (mut bench, root, set) = lab("compare-unconfined-link");
    write_png(&root.join("elsewhere/shot.png"));
    std::fs::create_dir_all(root.join(CAPTURES)).unwrap();
    let evil = root.join(CAPTURES).join("evil");
    assert!(link_dir(&evil, &root.join("elsewhere")), "the system refused to make a link");
    assert!(evil.join("shot.png").is_file(), "precondition: the link reaches the image");
    point_every_light_png_at(&mut bench.session, &set, ".uigen/captures/evil/shot.png");

    let result = bench.session.execute(COMPARE, &json!({"set": set})).unwrap();

    assert_every_row_is_missing(&result);
}

// covers: E5-3.7
#[test]
fn without_automation_roots_a_reference_written_with_dot_dot_segments_is_never_read() {
    let (mut bench, root, set) = lab("compare-unconfined-dotdot");
    write_png(&root.join("outside.png"));
    point_every_light_png_at(&mut bench.session, &set, ".uigen/captures/../../outside.png");

    let result = bench.session.execute(COMPARE, &json!({"set": set})).unwrap();

    assert_every_row_is_missing(&result);
}
