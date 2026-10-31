//! The exported Lit component against what it must look like: `export.lit` writes the package of a set, Chromium renders
//! the element in every variant (`sidecar/lit-shots.mjs`), and each shot is compared with a reference image. The bench
//! takes the library's own captures as references; the test below, on synthetic components, the canvas.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use figcraft_doc::NodeId;
use figcraft_engine::Session;
use figcraft_engine::uigen_compare::{Raster, aligned, library_raster, pixel_diff};
use serde_json::{Value, json};

use crate::locate;
use crate::service_tests::TempDir;

pub(crate) struct LitShots {
    folder: PathBuf,
    by_variant: BTreeMap<String, String>,
}

fn shot_name(id: &str) -> String {
    format!("{}.png", id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>())
}

fn run_shots(job: &Value, folder: &Path) -> Result<(), String> {
    let config = locate().map_err(|error| error.to_string())?;
    let script = config.script.with_file_name("lit-shots.mjs");
    let file = folder.join("job.json");
    fs::write(&file, job.to_string()).map_err(|error| error.to_string())?;
    let output = Command::new(&config.node).arg(&script).arg(&file).output().map_err(|error| error.to_string())?;
    if output.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&output.stderr).trim().to_string()) }
}

/// Exports the set into `folder` and renders every variant of its element in each theme.
pub(crate) fn shoot_set(session: &mut Session, set: &Value, folder: &Path, themes: &[&str]) -> Result<LitShots, String> {
    let package = folder.join("package");
    fs::create_dir_all(&package).map_err(|error| error.to_string())?;
    let exported = session
        .execute("export.lit", &json!({"ids": [set], "path": package.display().to_string(), "force": true}))
        .map_err(|error| error.to_string())?;
    let entry = exported["components"].as_array().and_then(|all| all.first()).ok_or_else(|| format!("nothing exported: {exported}"))?;
    let states = entry["states"].as_array().cloned().unwrap_or_default();
    let key = |state: &Value| state["id"].to_string();
    let variants: Vec<Value> = states.iter().map(|state| json!({"id": key(state), "attrs": state["attrs"], "pseudo": state["pseudo"]})).collect();
    let job = json!({
        "libDir": package.display().to_string(),
        "tag": entry["tag"],
        "variants": variants,
        "themes": themes,
        "outDir": folder.join("shots").display().to_string(),
    });
    run_shots(&job, folder)?;
    let by_variant = states.iter().map(|state| (key(state), shot_name(&key(state)))).collect();
    Ok(LitShots { folder: folder.join("shots"), by_variant })
}

impl LitShots {
    pub(crate) fn raster(&self, theme: &str, variant: NodeId) -> Result<Raster, String> {
        let name = self.by_variant.get(&json!(variant).to_string()).ok_or_else(|| format!("no shot of {variant}"))?;
        let image = image::open(self.folder.join(theme).join(name)).map_err(|error| error.to_string())?.to_rgba8();
        let (width, height) = image.dimensions();
        Raster::new(width, height, image.into_raw()).map_err(|error| format!("{error:?}"))
    }
}

/// The share of the pixels of both images that look the same, in percent, by the measure of `uigen.compare`.
pub(crate) fn lit_percent(reference: &Raster, lit: &Raster) -> f64 {
    let (first, second) = aligned(reference, lit);
    100.0 - pixel_diff(&first, &second, &[]).shapes_percent
}

fn command(session: &mut Session, name: &str, params: Value) -> Value {
    session.execute(name, &params).unwrap_or_else(|error| panic!("{name} {params}: {error}"))
}

fn node(value: &Value) -> NodeId {
    figcraft_engine::params::id_of(value).unwrap_or_else(|| panic!("no node in {value}"))
}

fn variant(session: &mut Session, name: &str, x: f64, props: Value) -> NodeId {
    let frame = command(session, "frame.create", json!({"x": x, "y": 0, "width": 120, "height": 40, "name": name}));
    command(session, "layout.add", json!({"ids": [frame["id"]]}));
    command(session, "node.set", json!({"id": frame["id"], "props": props}));
    command(session, "text.create", json!({"text": "Go", "x": 12, "y": 10, "parent": frame["id"], "props": {"name": "Label", "fill": "#112233"}}));
    node(&command(session, "component.create", json!({"ids": [frame["id"]]}))["id"])
}

#[test]
#[ignore = "drives Chromium; run by hand"]
fn every_variant_of_an_exported_set_looks_like_its_canvas_even_where_a_variant_drops_what_the_base_sets() {
    let dir = TempDir::new("lit-parity");
    let mut session = Session::empty();
    command(&mut session, "project.open", json!({"path": dir.path().display().to_string(), "create": true}));
    let solid = json!({"fill": "#cc2211", "cornerRadius": 8, "opacity": 0.8});
    let ghost = json!({"fills": [], "cornerRadius": 8});
    let ids = [
        variant(&mut session, "Tone=solid, State=Default", 0.0, solid),
        variant(&mut session, "Tone=solid, State=Hover", 200.0, json!({"fill": "#aa1100", "cornerRadius": 8})),
        variant(&mut session, "Tone=ghost, State=Default", 400.0, ghost.clone()),
        variant(&mut session, "Tone=ghost, State=Hover", 600.0, json!({"fill": "#eeeeee", "cornerRadius": 8})),
        variant(&mut session, "Tone=ghost, State=Pressed", 800.0, ghost),
    ];
    let set = command(&mut session, "component.combineVariants", json!({"ids": ids}));
    let shots = shoot_set(&mut session, &set["id"], &dir.path().join("lit"), &["light"]).unwrap_or_else(|error| panic!("{error}"));
    let doc = session.docs[session.active].doc.clone();
    for id in ids {
        let name = doc.node(id).map_or("", |n| n.name.as_str());
        let canvas = library_raster(&doc, id, false).unwrap_or_else(|error| panic!("{error}"));
        let lit = shots.raster("light", id).unwrap_or_else(|error| panic!("{error}"));
        let percent = lit_percent(&canvas, &lit);
        eprintln!("{name} lit={percent:.2}%");
        assert!(percent >= 90.0, "{name}: lit={percent:.2}%");
    }
}
