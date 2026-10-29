//! The Lit export plan: `export.lit` lists the components with their tag, class, file and issues,
//! and the files of the package (components, `tokens.css`, `index.ts`, `package.json`,
//! `tsconfig.json`, `README.md`) with their text, without touching the document or the disk;
//! `export.litWindow` only asks the app to open the export window.

use std::sync::Arc;

use crate::components::tests::{ex, idv};
use crate::{EngineError, Session};
use figcraft_doc::NodeId;
use serde_json::{Value, json};

const SENT_ANNOTATION: &str = "SentinelAnnotationText";
const SENT_RESOURCE: &str = "SentinelResourceName";
const SENT_RESOURCE_HOST: &str = "sentinelresource.example.com";
const SENT_NOTE: &str = "SentinelDevNoteText";
const SENT_PLUGIN_KEY: &str = "SentinelPluginKey";
const SENT_PLUGIN: &str = "SentinelPluginValue";
const SENT_DESCRIPTION: &str = "SentinelDescriptionText";
const SENT_EXPORT: &str = "SentinelExportSuffix";
const SENT_REFERENCE: &str = "SentinelReferenceText";
const SENT_MEASUREMENT: &str = "4321";

fn export(s: &mut Session, p: Value) -> Value {
    ex(s, "export.lit", p)
}

fn refusal(s: &mut Session, p: &Value) -> String {
    match s.execute("export.lit", p) {
        Err(EngineError::BadParams { cmd, msg }) => {
            assert_eq!(cmd, "export.lit");
            msg
        }
        other => panic!("export.lit {p}: expected a bad-parameters refusal, got {other:?}"),
    }
}

fn plain(s: &mut Session, name: &str, x: f64) -> NodeId {
    let f = idv(&ex(s, "frame.create", json!({"x": x, "y": 0, "width": 100, "height": 40, "name": name})));
    ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": {"name": "Bg", "fill": "#ff0000"}}));
    ex(s, "text.create", json!({"text": "Click", "x": 10, "y": 10, "parent": f, "props": {"name": "Label"}}));
    idv(&ex(s, "component.create", json!({"ids": [f]})))
}

fn hollow(s: &mut Session, name: &str, x: f64) -> NodeId {
    let f = idv(&ex(s, "frame.create", json!({"x": x, "y": 0, "width": 100, "height": 40, "name": name})));
    idv(&ex(s, "component.create", json!({"ids": [f]})))
}

fn variant_set(s: &mut Session, name: &str, prop: &str, values: &[&str]) -> NodeId {
    let mut variants = Vec::new();
    for (k, value) in values.iter().enumerate() {
        let f = idv(&ex(s, "frame.create", json!({"x": k as f64 * 150.0, "y": 100, "width": 100, "height": 40, "name": format!("{prop}={value}")})));
        let fill = if k == 0 { "#ff0000" } else { "#0000ff" };
        ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": {"name": "Bg", "fill": fill}}));
        variants.push(idv(&ex(s, "component.create", json!({"ids": [f]}))));
    }
    let set = idv(&ex(s, "component.combineVariants", json!({"ids": variants})));
    ex(s, "node.set", json!({"id": set, "props": {"name": name}}));
    set
}

fn colors(s: &mut Session) {
    ex(s, "variable.createCollection", json!({"name": "Colors", "modes": ["Light", "Dark"]}));
    ex(
        s,
        "variable.create",
        json!({"collection": "Colors", "name": "Colors/Brand 500", "type": "color", "values": {"Light": "#3366FF", "Dark": "#6699FF"}}),
    );
}

fn entries(v: &Value) -> &[Value] {
    v["components"].as_array().map(Vec::as_slice).unwrap_or_default()
}

fn entry_of(v: &Value, id: NodeId) -> &Value {
    entries(v).iter().find(|c| c["id"] == json!(id)).unwrap_or_else(|| panic!("no component {id:?} in {v}"))
}

fn issue_codes(entry: &Value) -> Vec<String> {
    entry["issues"].as_array().map(|all| all.iter().filter_map(|i| i["code"].as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn file<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    v["files"].as_array()?.iter().find(|f| f["path"] == path)
}

fn text_of(v: &Value, path: &str) -> String {
    file(v, path).and_then(|f| f["text"].as_str()).unwrap_or_else(|| panic!("no text for {path} in {v}")).to_string()
}

fn all_text(v: &Value) -> String {
    let files: Vec<&str> = v["files"].as_array().map(|all| all.iter().filter_map(|f| f["text"].as_str()).collect()).unwrap_or_default();
    format!("{}\n{}", files.join("\n"), v["notes"])
}

fn key_orders(json: &str) -> Vec<Vec<String>> {
    let mut finished = Vec::new();
    let mut open: Vec<Option<Vec<String>>> = Vec::new();
    let mut chars = json.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => open.push(Some(Vec::new())),
            '[' => open.push(None),
            '}' | ']' => {
                if let Some(Some(keys)) = open.pop() {
                    finished.push(keys);
                }
            }
            '"' => {
                let mut text = String::new();
                while let Some(ch) = chars.next() {
                    match ch {
                        '\\' => text.extend(chars.next()),
                        '"' => break,
                        other => text.push(other),
                    }
                }
                while chars.peek().is_some_and(|w| w.is_whitespace()) {
                    chars.next();
                }
                if chars.peek() == Some(&':')
                    && let Some(Some(keys)) = open.last_mut()
                {
                    keys.push(text);
                }
            }
            _ => {}
        }
    }
    finished
}

fn changed_nothing(s: &Session, before: (&Arc<figcraft_doc::Document>, (usize, usize))) {
    let ds = s.ds().unwrap();
    assert!(Arc::ptr_eq(&ds.doc, before.0), "the document is the very same one");
    assert_eq!(s.undo_depth(), before.1, "no undo step was added");
}

#[test]
fn three_sets_one_empty_give_two_ok_entries_and_one_empty_without_a_file() {
    // covers: E6-5.1
    let mut s = Session::new();
    let button = variant_set(&mut s, "Button", "Tone", &["Soft", "Loud"]);
    let card = plain(&mut s, "Card", 400.0);
    let ghost = hollow(&mut s, "Ghost", 800.0);
    let v = export(&mut s, json!({}));
    assert_eq!(entries(&v).len(), 3, "{v}");
    let b = entry_of(&v, button);
    assert_eq!((b["status"].as_str(), b["tag"].as_str(), b["className"].as_str()), (Some("ok"), Some("ui-button"), Some("UiButton")), "{b}");
    assert_eq!(b["file"], "components/ui-button.ts");
    let c = entry_of(&v, card);
    assert_eq!((c["status"].as_str(), c["tag"].as_str(), c["file"].as_str()), (Some("ok"), Some("ui-card"), Some("components/ui-card.ts")), "{c}");
    let g = entry_of(&v, ghost);
    assert_eq!(g["status"], "empty", "{g}");
    assert!(g["file"].is_null(), "an empty component has no file: {g}");
    let paths: Vec<&str> = v["files"].as_array().unwrap().iter().filter_map(|f| f["path"].as_str()).collect();
    assert!(paths.iter().all(|p| !p.contains("ghost")), "no file for the empty one: {paths:?}");
    assert!(file(&v, "components/ui-button.ts").is_some() && file(&v, "components/ui-card.ts").is_some(), "{paths:?}");
}

#[test]
fn the_package_lists_components_tokens_index_manifest_tsconfig_and_readme_with_their_text() {
    // covers: E6-5.2
    let mut s = Session::new();
    colors(&mut s);
    plain(&mut s, "Button", 0.0);
    let v = export(&mut s, json!({}));
    for path in ["components/ui-button.ts", "tokens/tokens.css", "index.ts", "package.json", "tsconfig.json", "README.md"] {
        let text = text_of(&v, path);
        assert!(!text.trim().is_empty(), "{path} has text");
    }
    let component = text_of(&v, "components/ui-button.ts");
    assert!(component.contains("@customElement(\"ui-button\")") && component.contains("export class UiButton"), "{component}");
    assert!(text_of(&v, "tokens/tokens.css").to_lowercase().contains("--ui-colors-brand-500: #3366ff;"));
    assert!(text_of(&v, "index.ts").contains("components/ui-button"), "the index reaches the component");
}

#[test]
fn package_json_is_valid_json_with_sorted_keys_and_the_validated_package_name() {
    // covers: E6-5.3
    let mut s = Session::new();
    plain(&mut s, "Button", 0.0);
    let default = export(&mut s, json!({}));
    let manifest: Value = serde_json::from_str(&text_of(&default, "package.json")).unwrap();
    assert_eq!(manifest["name"], "uigen-lib");
    assert_eq!(manifest["version"], "0.1.0");
    assert_eq!(manifest["type"], "module");
    assert!(manifest["main"].as_str().is_some_and(|m| !m.is_empty()) && manifest["types"].as_str().is_some_and(|t| !t.is_empty()), "{manifest}");
    assert!(manifest["dependencies"]["lit"].is_string() && manifest["devDependencies"]["typescript"].is_string(), "{manifest}");
    assert_eq!(manifest["scripts"]["build"], "tsc");
    let tsconfig: Value = serde_json::from_str(&text_of(&default, "tsconfig.json")).unwrap();
    assert_eq!(tsconfig["compilerOptions"]["experimentalDecorators"], true);
    assert_eq!(tsconfig["compilerOptions"]["useDefineForClassFields"], false);

    let named = export(&mut s, json!({"packageName": "acme-ui", "version": "2.3.4-beta.1"}));
    let text = text_of(&named, "package.json");
    let manifest: Value = serde_json::from_str(&text).unwrap();
    assert_eq!((manifest["name"].as_str(), manifest["version"].as_str()), (Some("acme-ui"), Some("2.3.4-beta.1")));
    let orders = key_orders(&text);
    assert!(orders.last().is_some_and(|top| top.len() >= 6), "the top level is the last object closed: {orders:?}");
    for keys in &orders {
        assert!(keys.windows(2).all(|w| w[0] <= w[1]), "keys are sorted: {keys:?} in {text}");
    }
    let scoped = export(&mut s, json!({"packageName": "@acme/ui-kit"}));
    assert_eq!(serde_json::from_str::<Value>(&text_of(&scoped, "package.json")).unwrap()["name"], "@acme/ui-kit");

    for name in ["Bad Name", "UPPER", ".hidden", "_under", "../evil", "a b", &"x".repeat(215)] {
        refusal(&mut s, &json!({"packageName": name}));
    }
}

#[test]
fn names_that_make_no_valid_tag_are_reported_and_block_a_write_until_renamed() {
    // covers: E6-5.4
    let mut s = Session::new();
    let nameless = plain(&mut s, "###", 0.0);
    let digit = plain(&mut s, "3D Card", 200.0);
    let first = plain(&mut s, "Primary Button", 400.0);
    let second = plain(&mut s, "Primary Button", 600.0);
    let reserved = plain(&mut s, "Font Face", 800.0);
    let v = export(&mut s, json!({"prefix": ""}));
    assert_eq!(v["ready"], false, "{v}");
    assert!(issue_codes(entry_of(&v, nameless)).contains(&"empty".to_string()), "{v}");
    assert!(issue_codes(entry_of(&v, digit)).contains(&"leadingDigit".to_string()), "{v}");
    assert!(issue_codes(entry_of(&v, reserved)).contains(&"reserved".to_string()), "{v}");
    let twins: Vec<String> = [first, second].into_iter().flat_map(|id| issue_codes(entry_of(&v, id))).collect();
    assert!(twins.contains(&"duplicate".to_string()), "{v}");
    for id in [nameless, digit, reserved] {
        let e = entry_of(&v, id);
        assert_eq!(e["status"], "invalid", "{e}");
        assert!(e["issues"][0]["message"].as_str().is_some_and(|m| !m.is_empty()), "an issue says what is wrong: {e}");
    }

    let dir = std::env::temp_dir().join(format!("figcraft-lit-export-blocked-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let message = refusal(&mut s, &json!({"prefix": "", "path": dir.to_string_lossy()}));
    for name in ["3D Card", "Font Face", "Primary Button"] {
        assert!(message.contains(name), "the refusal names {name}: {message}");
    }
    let written = std::fs::read_dir(&dir).unwrap().count();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(written, 0, "nothing was written");

    for (id, name) in [(nameless, "Empty Name"), (digit, "Card 3D"), (second, "Secondary Button"), (reserved, "Fancy Face")] {
        ex(&mut s, "node.set", json!({"id": id, "props": {"name": name}}));
    }
    let v = export(&mut s, json!({"prefix": ""}));
    assert_eq!(v["ready"], true, "{v}");
    assert!(entries(&v).iter().all(|e| e["status"] == "ok" && issue_codes(e).is_empty()), "{v}");
}

#[test]
fn dev_only_data_and_a_locked_reference_frame_never_reach_a_file_or_a_note() {
    // covers: E6-5.5
    let mut s = Session::new();
    let reference = idv(&ex(&mut s, "frame.create", json!({"x": 4421, "y": 0, "width": 300, "height": 300, "name": "Référence"})));
    let target =
        idv(&ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 50, "height": 50, "parent": reference, "props": {"name": "Target"}})));
    ex(&mut s, "text.create", json!({"text": SENT_REFERENCE, "x": 60, "y": 0, "parent": reference, "props": {"name": "Spec"}}));
    let comp = plain(&mut s, "Panel", 0.0);
    let bg = s.doc().unwrap().node(comp).unwrap().children[0];
    ex(&mut s, "annotation.add", json!({"id": bg, "label": SENT_ANNOTATION}));
    ex(&mut s, "devResource.add", json!({"id": comp, "url": format!("https://{SENT_RESOURCE_HOST}/docs"), "name": SENT_RESOURCE}));
    ex(&mut s, "node.set", json!({"id": comp, "props": {"devStatus": "readyForDev"}}));
    ex(&mut s, "node.set", json!({"id": bg, "props": {"height": 44}}));
    ex(&mut s, "dev.resolveChange", json!({"ids": [comp], "reason": SENT_NOTE}));
    ex(&mut s, "devMode.addMeasurement", json!({"from": bg, "to": target}));
    let labels = ex(&mut s, "devMode.measurements", json!({"id": bg})).to_string();
    assert!(labels.contains(SENT_MEASUREMENT), "the measurement reads {SENT_MEASUREMENT}: {labels}");
    ex(&mut s, "node.set", json!({"ids": [comp, bg], "props": {"pluginData": {SENT_PLUGIN_KEY: SENT_PLUGIN}}}));
    ex(&mut s, "node.set", json!({"id": comp, "props": {"description": SENT_DESCRIPTION}}));
    ex(&mut s, "export.add", json!({"ids": [bg], "setting": {"size": "1x", "suffix": SENT_EXPORT, "format": "png"}}));
    ex(&mut s, "node.set", json!({"id": reference, "props": {"locked": true}}));

    let v = export(&mut s, json!({}));
    assert!(text_of(&v, "components/ui-panel.ts").contains("export class UiPanel"), "the component itself is exported: {v}");
    let everything = all_text(&v);
    for sentinel in [
        SENT_ANNOTATION,
        SENT_RESOURCE,
        SENT_RESOURCE_HOST,
        SENT_NOTE,
        SENT_PLUGIN_KEY,
        SENT_PLUGIN,
        SENT_DESCRIPTION,
        SENT_EXPORT,
        SENT_REFERENCE,
        SENT_MEASUREMENT,
    ] {
        assert!(!everything.contains(sentinel), "{sentinel} leaked into the export:\n{everything}");
    }
}

#[test]
fn a_component_that_uses_another_exports_it_as_a_dependency_and_renders_its_tag() {
    // covers: E6-5.6
    let mut s = Session::new();
    let icon = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 400, "width": 24, "height": 24, "name": "Heart"})));
    ex(&mut s, "shape.star", json!({"x": 0, "y": 0, "width": 24, "height": 24, "parent": icon}));
    let heart = idv(&ex(&mut s, "component.create", json!({"ids": [icon]})));
    let f = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 120, "height": 40, "name": "Chip"})));
    ex(&mut s, "instance.create", json!({"component": heart, "x": 8, "y": 8, "parent": f}));
    let chip = idv(&ex(&mut s, "component.create", json!({"ids": [f]})));
    let v = export(&mut s, json!({"ids": [chip]}));
    assert_eq!(entry_of(&v, chip)["status"], "ok", "{v}");
    let dependency = entry_of(&v, heart);
    assert_eq!(dependency["status"], "dependency", "{dependency}");
    assert!(text_of(&v, "components/ui-heart.ts").contains("export class UiHeart"), "the dependency is exported too");
    assert!(text_of(&v, "components/ui-chip.ts").contains("<ui-heart"), "render() holds the dependency's tag");
    let notes = v["notes"].as_array().unwrap();
    assert!(notes.iter().any(|n| n.to_string().to_lowercase().contains("heart")), "a note mentions the dependency: {notes:?}");
}

#[test]
fn exporting_twice_gives_the_same_files_in_sorted_order() {
    // covers: E6-5.7
    let mut s = Session::new();
    colors(&mut s);
    plain(&mut s, "Zebra", 0.0);
    plain(&mut s, "Alpha", 200.0);
    let first = export(&mut s, json!({}));
    let second = export(&mut s, json!({}));
    assert_eq!(first["files"], second["files"]);
    let index = text_of(&first, "index.ts");
    let (alpha, zebra) = (index.find("ui-alpha"), index.find("ui-zebra"));
    assert!(alpha.is_some() && zebra.is_some() && alpha < zebra, "the index is sorted, not in creation order:\n{index}");
}

#[test]
fn ids_restrict_the_export_and_bad_ids_or_prefixes_are_refused() {
    // covers: E6-5.8
    let mut s = Session::new();
    let alpha = plain(&mut s, "Alpha", 0.0);
    plain(&mut s, "Zebra", 200.0);
    let rect = idv(&ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 200, "width": 10, "height": 10})));
    let v = export(&mut s, json!({"ids": [alpha]}));
    assert_eq!(entries(&v).len(), 1, "{v}");
    assert_eq!(entry_of(&v, alpha)["tag"], "ui-alpha");
    assert!(file(&v, "components/ui-alpha.ts").is_some() && file(&v, "components/ui-zebra.ts").is_none(), "{v}");

    refusal(&mut s, &json!({"ids": [987_654]}));
    refusal(&mut s, &json!({"ids": [alpha, rect]}));
    for bad in ["Acme", "a b", "a\"b"] {
        refusal(&mut s, &json!({"prefix": bad}));
        refusal(&mut s, &json!({"tokenPrefix": bad}));
    }
}

#[test]
fn the_plan_leaves_the_document_and_its_history_alone() {
    // covers: E6-5.9
    let mut s = Session::new();
    colors(&mut s);
    let alpha = plain(&mut s, "Alpha", 0.0);
    plain(&mut s, "Zebra", 200.0);
    let before = s.ds().unwrap().doc.clone();
    let depth = s.undo_depth();
    export(&mut s, json!({}));
    changed_nothing(&s, (&before, depth));
    export(&mut s, json!({"ids": [alpha], "prefix": "acme-", "packageName": "acme-ui"}));
    changed_nothing(&s, (&before, depth));
    refusal(&mut s, &json!({"ids": [987_654]}));
    changed_nothing(&s, (&before, depth));
}

#[test]
fn absurd_parameters_never_panic_and_write_nothing() {
    // covers: E6-5.10
    let junk = [
        json!(null),
        json!([1, 2, 3]),
        json!({"id": "nope", "x": "NaN", "props": 5}),
        json!({"ids": "x"}),
        json!({"ids": ["x", -1, 99999]}),
        json!({"ids": [null, {"id": 1}]}),
        json!({"prefix": 5}),
        json!({"prefix": "\u{0}"}),
        json!({"prefix": "x".repeat(10_000)}),
        json!({"tokenPrefix": [1]}),
        json!({"tokenPrefix": "\u{7}"}),
        json!({"collections": [1, null, "nope"]}),
        json!({"collections": "Colors"}),
        json!({"lightMode": {"x": 1}, "darkMode": 7}),
        json!({"packageName": 7}),
        json!({"packageName": "\u{0}"}),
        json!({"version": ["1"]}),
        json!({"version": "1e999.-1.x"}),
        json!({"force": "yes"}),
        json!({"path": [5]}),
        json!({"path": null}),
        json!({"path": 7, "force": true}),
    ];
    let mut s = Session::new();
    colors(&mut s);
    plain(&mut s, "Button", 0.0);
    hollow(&mut s, "Ghost", 200.0);
    assert_eq!(export(&mut s, json!({}))["components"].as_array().map(Vec::len), Some(2), "the command exists and answers a plain call");
    ex(&mut s, "export.litWindow", json!({}));
    for command in ["export.lit", "export.litWindow"] {
        for j in &junk {
            if let Err(EngineError::Internal { cmd, msg }) = s.execute(command, j) {
                panic!("{cmd} panicked on {j}: {msg}");
            }
        }
    }
}

#[test]
fn the_window_command_only_asks_the_app_to_open_the_export_window() {
    // covers: E6-5.11
    let mut s = Session::new();
    plain(&mut s, "Button", 0.0);
    s.ui_requests.clear();
    let before = s.ds().unwrap().doc.clone();
    let depth = s.undo_depth();
    ex(&mut s, "export.litWindow", json!({}));
    assert_eq!(s.ui_requests.len(), 1, "{:?}", s.ui_requests);
    assert_eq!((s.ui_requests[0].kind.as_str(), &s.ui_requests[0].data), ("export.lit", &json!({})));
    changed_nothing(&s, (&before, depth));
}
