//! A repeatable measure of how close imported components come to their originals: it imports a fixed
//! list of components into an existing library project and prints `uigen.compare` for every variant.
//! Parity % = 100 × (1 − differing pixels / pixels of the larger raster), all pixels of the variant, text
//! included: differing = `shapes.differing` + `text.pixelPercent` of the pixels inside the text boxes.
//! Shapes % is the same outside the text boxes only, the figure Compare judges. Every case runs in light and dark.
//! `lit=` % compares the original with the exported Lit element rendered in Chromium, all pixels, per variant.
//! `#[ignore]`d: it needs Node, Chromium and a project whose `.uigen/sources` holds the packages.
//!
//! ```text
//! $env:UIGEN_BENCH_PROJECT = '<a copy of a project: uigen.json, its design file, .uigen/sources>'
//! $env:UIGEN_BENCH_PACKAGE = '@heroui/react'   # optional: only the cases of this package
//! $env:UIGEN_BENCH_ONLY = 'Button,Card'        # optional: only these components
//! $env:UIGEN_BENCH_OUT = '<folder>'            # optional: original | library | difference PNG per variant
//! cargo test -p figcraft-capture --offline fidelity_bench -- --ignored --nocapture
//! ```

use std::env;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use figcraft_doc::{Document, NodeId};
use figcraft_engine::Session;
use figcraft_engine::uigen_compare::{Raster, difference_image, library_raster, original_raster};
use serde_json::{Value, json};

use crate::lit_parity_tests::{LitShots, lit_percent, shoot_set};
use crate::{CaptureService, locate};

const CASES: &str = include_str!("../fixtures/fidelity-cases.json");
const IMPORT: &str = "uigen.importComponent";
const COMPARE: &str = "uigen.compare";
const JOB_LIMIT: Duration = Duration::from_secs(600);
const GAP: u32 = 8;
const THEMES: [&str; 2] = ["light", "dark"];

struct Bench {
    session: Session,
    root: String,
    out: Option<PathBuf>,
    scores: Vec<f64>,
    lit_scores: Vec<f64>,
}

fn variable(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn wanted(case: &Value) -> bool {
    let of = |key: &str| case[key].as_str().unwrap_or_default();
    variable("UIGEN_BENCH_PACKAGE").is_none_or(|package| package == of("package"))
        && variable("UIGEN_BENCH_ONLY").is_none_or(|only| only.split(',').any(|name| name.trim() == of("component")))
}

fn import_params(case: &Value) -> Value {
    let mut params =
        json!({"package": case["package"], "component": case["component"], "states": case["states"], "animations": false, "themes": THEMES});
    for key in ["styles", "sizes"] {
        if case[key].as_array().is_some_and(|values| !values.is_empty()) {
            params[key] = case[key].clone();
        }
    }
    params
}

fn import(session: &mut Session, case: &Value) -> Result<Value, String> {
    let started = session.execute(IMPORT, &import_params(case)).map_err(|error| error.to_string())?;
    let job = started["job"].as_u64().ok_or_else(|| format!("no job in {started}"))?;
    let deadline = Instant::now() + JOB_LIMIT;
    while Instant::now() < deadline {
        if let Some(event) = session.pump_capture().into_iter().find(|event| event.job == job) {
            return event.outcome.map_err(|error| error.message);
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!("no import event within {JOB_LIMIT:?}"))
}

fn parity(row: &Value) -> f64 {
    let side = |key: &str| row["original"][key].as_f64().unwrap_or(0.0).max(row["library"][key].as_f64().unwrap_or(0.0));
    let all = side("width") * side("height");
    let number = |value: &Value| value.as_f64().unwrap_or(0.0);
    let text_pixels = (all - number(&row["shapes"]["total"])).max(0.0);
    let differing = number(&row["shapes"]["differing"]) + number(&row["text"]["pixelPercent"]) / 100.0 * text_pixels;
    if all > 0.0 { 100.0 * (1.0 - differing / all) } else { 0.0 }
}

/// The shots of the exported set of a case, in the folder of the bench inside the project.
fn lit_shots(session: &mut Session, root: &str, title: &str, set: &Value) -> Option<LitShots> {
    let folder = Path::new(root).join(".uigen").join("lit-bench").join(safe_name(title));
    match shoot_set(session, set, &folder, &THEMES) {
        Ok(shots) => Some(shots),
        Err(message) => {
            println!("{title}: LIT EXPORT FAILED: {message}");
            None
        }
    }
}

fn side_by_side(parts: &[&Raster]) -> image::RgbaImage {
    let width = parts.iter().map(|raster| raster.width + GAP).sum::<u32>().max(1);
    let height = parts.iter().map(|raster| raster.height).max().unwrap_or(1).max(1);
    let mut sheet = image::RgbaImage::from_pixel(width, height, image::Rgba([128, 128, 128, 255]));
    let mut left = 0;
    for raster in parts {
        if let Some(part) = image::RgbaImage::from_raw(raster.width, raster.height, raster.rgba.clone()) {
            image::imageops::overlay(&mut sheet, &part, i64::from(left), 0);
        }
        left += raster.width + GAP;
    }
    sheet
}

fn safe_name(text: &str) -> String {
    text.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

fn save_sheet(doc: &Document, root: &str, out: &Path, (title, theme): (&str, &str), variant: NodeId) -> Result<(), String> {
    let reference = figcraft_engine::reference_of(doc, variant).ok_or("no reference")?;
    let png = reference.themes.get(theme).and_then(|kept| kept.png.clone()).ok_or("no original png")?;
    let original = original_raster(root, &png)?;
    let library = library_raster(doc, variant, theme == "dark")?;
    let difference = difference_image(&original, &library);
    std::fs::create_dir_all(out).map_err(|error| error.to_string())?;
    side_by_side(&[&original, &library, &difference]).save(out.join(format!("{}.png", safe_name(title)))).map_err(|error| error.to_string())
}

fn reference_png(doc: &Document, root: &str, variant: NodeId, theme: &str) -> Result<Raster, String> {
    let reference = figcraft_engine::reference_of(doc, variant).ok_or("no reference")?;
    let png = reference.themes.get(theme).and_then(|kept| kept.png.clone()).ok_or("no original png")?;
    original_raster(root, &png).map_err(str::to_string)
}

fn variant_id(doc: &Document, set: NodeId, name: &str) -> Option<NodeId> {
    doc.node(set)?.children.iter().copied().find(|id| doc.node(*id).is_some_and(|node| node.name == name))
}

impl Bench {
    fn run_case(&mut self, case: &Value) {
        let title = format!("{} {}", case["package"].as_str().unwrap_or_default(), case["component"].as_str().unwrap_or_default());
        let imported = match import(&mut self.session, case) {
            Ok(imported) => imported,
            Err(message) => return println!("{title}: IMPORT FAILED: {}", message.lines().next().unwrap_or_default()),
        };
        let shots = lit_shots(&mut self.session, &self.root, &title, &imported["set"]);
        for theme in THEMES {
            let compared = match self.session.execute(COMPARE, &json!({"set": imported["set"], "theme": theme})) {
                Ok(compared) => compared,
                Err(error) => {
                    println!("{title} {theme}: COMPARE FAILED: {error}");
                    continue;
                }
            };
            for row in compared["rows"].as_array().into_iter().flatten() {
                let lit = shots.as_ref().and_then(|shots| self.lit_score(shots, &imported, row, theme));
                self.print_row((&title, theme), &imported, row, lit);
            }
        }
        for warning in imported["warnings"].as_array().into_iter().flatten() {
            println!("  note {}", warning.as_str().unwrap_or_default());
        }
    }

    fn lit_score(&mut self, shots: &LitShots, imported: &Value, row: &Value, theme: &str) -> Option<f64> {
        let doc = self.session.docs[self.session.active].doc.as_ref();
        let id = figcraft_engine::params::id_of(&imported["set"]).and_then(|set| variant_id(doc, set, row["variant"].as_str()?))?;
        let reference = reference_png(doc, &self.root, id, theme).ok()?;
        let lit = shots.raster(theme, id).ok()?;
        let score = lit_percent(&reference, &lit);
        self.lit_scores.push(score);
        if let Some(out) = &self.out {
            let difference = difference_image(&reference, &lit);
            let name = format!("{}.png", safe_name(&format!("lit {theme} {}", row["variant"].as_str()?)));
            let sheet = side_by_side(&[&reference, &lit, &difference]);
            if std::fs::create_dir_all(out).and_then(|()| sheet.save(out.join(&name)).map_err(std::io::Error::other)).is_err() {
                println!("  no Lit sheet for {name}");
            }
        }
        Some(score)
    }

    fn print_row(&mut self, (title, theme): (&str, &str), imported: &Value, row: &Value, lit: Option<f64>) {
        let variant = row["variant"].as_str().unwrap_or_default();
        let size = |side: &Value| format!("{}x{}", side["width"], side["height"]);
        match row["shapes"]["percent"].as_f64() {
            Some(percent) => {
                let score = parity(row);
                let lit = lit.map_or_else(|| "n/a".to_string(), |lit| format!("{lit:.2}%"));
                self.scores.push(score);
                println!(
                    "{score:7.2} %  {title} {theme} [{variant}] shapes={:.2}% status={} original={} library={} text={} textPixels={:.2}% lit={lit}",
                    100.0 - percent,
                    row["status"].as_str().unwrap_or_default(),
                    size(&row["original"]),
                    size(&row["library"]),
                    row["text"]["verdict"].as_str().unwrap_or_default(),
                    row["text"]["pixelPercent"].as_f64().unwrap_or(f64::NAN),
                );
            }
            None => println!("    n/a    {title} {theme} [{variant}] status={} reason={}", row["status"].as_str().unwrap_or_default(), row["reason"]),
        }
        if let Some(out) = &self.out {
            let doc = self.session.docs[self.session.active].doc.as_ref();
            let id = figcraft_engine::params::id_of(&imported["set"]).and_then(|set| variant_id(doc, set, variant));
            let saved = id
                .ok_or_else(|| "no variant".to_string())
                .and_then(|id| save_sheet(doc, &self.root, out, (&format!("{title} {theme} {variant}"), theme), id));
            if let Err(error) = saved {
                println!("  no sheet for {variant}: {error}");
            }
        }
    }
}

#[test]
#[ignore = "needs Node, Chromium and UIGEN_BENCH_PROJECT; run by hand"]
fn fidelity_bench() {
    let root = variable("UIGEN_BENCH_PROJECT").expect("UIGEN_BENCH_PROJECT names a copy of a library project");
    let config = locate().expect("node and the sidecar script must be found (UIGEN_NODE, UIGEN_SIDECAR_DIR)");
    let mut session = Session::empty();
    session.set_capture_backend(Box::new(CaptureService::start(config, Box::new(|| {}))));
    let opened = session.execute("project.open", &json!({"path": root})).unwrap_or_else(|error| panic!("{root}: {error}"));
    assert_eq!(opened["opened"], json!(true), "{opened}");
    let cases: Vec<Value> = serde_json::from_str(CASES).expect("fixtures/fidelity-cases.json is valid");
    let mut bench = Bench { session, root, out: variable("UIGEN_BENCH_OUT").map(PathBuf::from), scores: Vec::new(), lit_scores: Vec::new() };
    for case in cases.iter().filter(|case| wanted(case)) {
        bench.run_case(case);
    }
    let measured = bench.scores.len();
    let mean = bench.scores.iter().sum::<f64>() / measured.max(1) as f64;
    let lowest = bench.scores.iter().copied().fold(100.0, f64::min);
    println!("summary: {measured} variants measured, mean {mean:.2} %, lowest {lowest:.2} %");
    let lit_mean = bench.lit_scores.iter().sum::<f64>() / bench.lit_scores.len().max(1) as f64;
    let lit_lowest = bench.lit_scores.iter().copied().fold(100.0, f64::min);
    println!("lit: {} variants measured, mean {lit_mean:.2} %, lowest {lit_lowest:.2} %", bench.lit_scores.len());
}
