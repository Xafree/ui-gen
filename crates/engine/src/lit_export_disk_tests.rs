//! Writing the Lit package to a folder: `export.lit` with a `path` writes the planned files and
//! `.uigen/export.json`, a manifest of what it generated and the hash of each file. A later export
//! replaces only the files it generated and nobody edited, and says what it kept, skipped or left
//! behind.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::components::tests::{ex, idv};
use crate::file_access::{Roots, confined};
use crate::{EngineError, Session};
use figcraft_doc::NodeId;
use serde_json::{Value, json};

const MANIFEST: &str = ".uigen/export.json";
const BUTTON: &str = "components/ui-button.ts";
const CARD: &str = "components/ui-card.ts";
const GENERATED: [&str; 7] = [BUTTON, CARD, "tokens/tokens.css", "index.ts", "package.json", "tsconfig.json", "README.md"];

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-lit-disk-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }

    fn put(&self, rel: &str, bytes: &[u8]) {
        let path = self.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut found = BTreeMap::new();
        collect(&self.0, &self.0, &mut found);
        found
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn collect(root: &Path, at: &Path, found: &mut BTreeMap<String, Vec<u8>>) {
    for entry in std::fs::read_dir(at).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, found);
        } else {
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            found.insert(rel, std::fs::read(&path).unwrap());
        }
    }
}

fn component(s: &mut Session, name: &str, x: f64) -> (NodeId, NodeId) {
    let f = idv(&ex(s, "frame.create", json!({"x": x, "y": 0, "width": 100, "height": 40, "name": name})));
    let bg =
        idv(&ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": {"name": "Bg", "fill": "#ff0000"}})));
    ex(s, "text.create", json!({"text": "Click", "x": 10, "y": 10, "parent": f, "props": {"name": "Label"}}));
    (idv(&ex(s, "component.create", json!({"ids": [f]}))), bg)
}

fn button_and_card(s: &mut Session) -> (NodeId, NodeId, NodeId) {
    let (button, bg) = component(s, "Button", 0.0);
    let (card, _) = component(s, "Card", 200.0);
    (button, bg, card)
}

fn export_to(s: &mut Session, dir: &Folder, extra: Value) -> Value {
    let mut params = json!({"path": dir.path()});
    if let (Some(all), Some(more)) = (params.as_object_mut(), extra.as_object()) {
        all.extend(more.clone());
    }
    ex(s, "export.lit", params)
}

fn planned_text(s: &mut Session, path: &str) -> String {
    let plan = ex(s, "export.lit", json!({}));
    let file = plan["files"].as_array().unwrap().iter().find(|f| f["path"] == path).unwrap_or_else(|| panic!("{path} is not planned"));
    file["text"].as_str().unwrap().to_string()
}

fn list(v: &Value, key: &str) -> Vec<String> {
    match v.get(key) {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(all)) => all.iter().map(|p| p.as_str().unwrap_or_else(|| panic!("{key} holds paths: {v}")).to_string()).collect(),
        Some(other) => panic!("{key} must be a list of paths: {other}"),
    }
}

fn skipped(v: &Value) -> Vec<(String, String)> {
    let all = match v.get("skipped") {
        None | Some(Value::Null) => return Vec::new(),
        Some(Value::Array(all)) => all,
        Some(other) => panic!("skipped must be a list: {other}"),
    };
    let field = |e: &Value, k: &str| e[k].as_str().unwrap_or_else(|| panic!("skipped entries have a {k}: {e}")).to_string();
    all.iter().map(|e| (field(e, "path"), field(e, "reason"))).collect()
}

fn written_but_the_manifest(v: &Value) -> Vec<String> {
    list(v, "written").into_iter().filter(|p| p != MANIFEST).collect()
}

fn manifest_of(dir: &Folder) -> Value {
    serde_json::from_slice(&dir.read(MANIFEST)).unwrap()
}

fn sha_of(manifest: &Value, path: &str) -> String {
    let files = manifest["files"].as_array().unwrap();
    let entry = files.iter().find(|f| f["path"] == path).unwrap_or_else(|| panic!("{path} is not in the manifest: {manifest}"));
    entry["sha256"].as_str().unwrap().to_string()
}

fn recolor(s: &mut Session, bg: NodeId, color: &str) {
    ex(s, "node.set", json!({"id": bg, "props": {"fill": color}}));
}

#[test]
fn the_first_export_into_an_empty_folder_writes_the_whole_package_and_lists_it() {
    // covers: E6-6.1
    let mut s = Session::new();
    button_and_card(&mut s);
    let dir = Folder::new("first");
    let v = export_to(&mut s, &dir, json!({}));
    let mut every: Vec<&str> = GENERATED.to_vec();
    every.push(MANIFEST);
    every.sort_unstable();
    let on_disk = dir.snapshot();
    assert_eq!(on_disk.keys().map(String::as_str).collect::<Vec<_>>(), every, "exactly the package and its manifest: {v}");
    let mut written = list(&v, "written");
    written.sort();
    assert_eq!(written, every, "{v}");
    assert!(list(&v, "unchanged").is_empty() && skipped(&v).is_empty() && list(&v, "stale").is_empty(), "{v}");
    for path in GENERATED {
        assert_eq!(dir.read(path), planned_text(&mut s, path).into_bytes(), "{path} holds the planned text");
    }
}

#[test]
fn the_manifest_is_version_1_json_with_a_sorted_sha256_per_generated_file() {
    // covers: E6-6.2
    let mut s = Session::new();
    button_and_card(&mut s);
    let dir = Folder::new("manifest");
    export_to(&mut s, &dir, json!({}));
    let manifest = manifest_of(&dir);
    assert_eq!(manifest["version"], 1, "{manifest}");
    assert_eq!(manifest["generator"], "uigen-lit", "{manifest}");
    let files = manifest["files"].as_array().unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f["path"].as_str().unwrap()).collect();
    let mut sorted = GENERATED.to_vec();
    sorted.sort_unstable();
    assert_eq!(paths, sorted, "every generated file, sorted, and not the manifest itself");
    for f in files {
        let sha = f["sha256"].as_str().unwrap();
        assert!(sha.len() == 64 && sha.chars().all(|c| c.is_ascii_hexdigit()), "{f}");
    }
    let distinct: std::collections::BTreeSet<&str> = files.iter().filter_map(|f| f["sha256"].as_str()).collect();
    assert_eq!(distinct.len(), files.len(), "each file has the hash of its own content");
}

#[test]
fn changing_one_component_replaces_only_its_file() {
    // covers: E6-6.3
    let mut s = Session::new();
    let (_, bg, _) = button_and_card(&mut s);
    let dir = Folder::new("one-file");
    export_to(&mut s, &dir, json!({}));
    let (before, before_manifest) = (dir.snapshot(), manifest_of(&dir));

    let same = export_to(&mut s, &dir, json!({}));
    assert!(written_but_the_manifest(&same).is_empty(), "nothing changed, nothing rewritten: {same}");
    let unchanged = list(&same, "unchanged");
    assert!(GENERATED.iter().all(|p| unchanged.iter().any(|u| u == p)), "{same}");

    recolor(&mut s, bg, "#0000ff");
    let v = export_to(&mut s, &dir, json!({}));
    assert_eq!(written_but_the_manifest(&v), [BUTTON], "{v}");
    let unchanged = list(&v, "unchanged");
    for other in GENERATED.iter().filter(|p| **p != BUTTON) {
        assert!(unchanged.iter().any(|u| u == other), "{other} is unchanged: {v}");
        assert_eq!(dir.read(other), before[*other], "{other} is untouched");
    }
    assert!(String::from_utf8(dir.read(BUTTON)).unwrap().to_lowercase().contains("0000ff"));
    assert_ne!(dir.read(BUTTON), before[BUTTON]);
    let manifest = manifest_of(&dir);
    assert_ne!(sha_of(&manifest, BUTTON), sha_of(&before_manifest, BUTTON));
    assert_eq!(sha_of(&manifest, CARD), sha_of(&before_manifest, CARD));
}

#[test]
fn files_added_by_hand_are_never_touched_even_with_force() {
    // covers: E6-6.4
    let mut s = Session::new();
    let (_, bg, _) = button_and_card(&mut s);
    let dir = Folder::new("by-hand");
    export_to(&mut s, &dir, json!({}));
    let mine: [(&str, &[u8]); 3] =
        [("components/custom.ts", b"export const mine = 1;\r\n\xff\xfe"), ("notes.md", b"# notes\n"), ("docs/deep/guide.txt", b"guide")];
    for (path, bytes) in mine {
        dir.put(path, bytes);
    }
    recolor(&mut s, bg, "#0000ff");
    let v = export_to(&mut s, &dir, json!({}));
    let kept = list(&v, "kept");
    assert!(kept.iter().any(|k| k == "components/custom.ts") && kept.iter().any(|k| k == "notes.md"), "{v}");
    let forced = export_to(&mut s, &dir, json!({"force": true}));
    assert!(list(&forced, "kept").iter().any(|k| k == "notes.md"), "{forced}");
    for (path, bytes) in mine {
        assert_eq!(dir.read(path), bytes, "{path} is identical to the byte");
    }
}

#[test]
fn a_generated_file_edited_by_hand_is_skipped_until_force_replaces_it() {
    // covers: E6-6.5
    let mut s = Session::new();
    button_and_card(&mut s);
    let dir = Folder::new("edited");
    export_to(&mut s, &dir, json!({}));
    dir.put(BUTTON, b"// my own version\n");
    let v = export_to(&mut s, &dir, json!({}));
    assert_eq!(dir.read(BUTTON), b"// my own version\n", "the edit survives");
    assert!(skipped(&v).contains(&(BUTTON.to_string(), "edited".to_string())), "{v}");
    assert!(!list(&v, "written").iter().any(|p| p == BUTTON), "{v}");

    let forced = export_to(&mut s, &dir, json!({"force": true}));
    assert_eq!(dir.read(BUTTON), planned_text(&mut s, BUTTON).into_bytes(), "force brings the generated file back");
    assert!(list(&forced, "written").iter().any(|p| p == BUTTON) && skipped(&forced).is_empty(), "{forced}");
}

#[test]
fn a_file_the_export_did_not_generate_is_kept_under_a_generated_name_until_force() {
    // covers: E6-6.6
    let mut s = Session::new();
    button_and_card(&mut s);
    let dir = Folder::new("foreign");
    dir.put(BUTTON, b"// somebody else's button\n");
    dir.put("README.md", b"my readme\n");
    let v = export_to(&mut s, &dir, json!({}));
    assert_eq!(dir.read(BUTTON), b"// somebody else's button\n");
    assert_eq!(dir.read("README.md"), b"my readme\n");
    let skipped = skipped(&v);
    for path in [BUTTON, "README.md"] {
        assert!(skipped.contains(&(path.to_string(), "not-generated".to_string())), "{path}: {v}");
    }
    for path in [CARD, "index.ts", "package.json"] {
        assert!(dir.join(path).is_file(), "the free names are still written: {path}");
    }

    export_to(&mut s, &dir, json!({"force": true}));
    assert_eq!(dir.read(BUTTON), planned_text(&mut s, BUTTON).into_bytes());
}

#[test]
fn a_removed_component_leaves_its_file_in_place_and_is_reported_stale() {
    // covers: E6-6.7
    let mut s = Session::new();
    let (_, _, card) = button_and_card(&mut s);
    let dir = Folder::new("stale");
    export_to(&mut s, &dir, json!({}));
    let card_before = dir.read(CARD);
    ex(&mut s, "edit.delete", json!({"ids": [card]}));
    let v = export_to(&mut s, &dir, json!({}));
    assert_eq!(dir.read(CARD), card_before, "never deleted, never rewritten");
    assert_eq!(list(&v, "stale"), [CARD], "{v}");
    assert!(!list(&v, "written").iter().any(|p| p == CARD), "{v}");
}

#[test]
fn an_unreadable_or_foreign_version_manifest_counts_as_absent_and_overwrites_nothing() {
    // covers: E6-6.8
    let foreign = |real: &Value| {
        let mut other = real.clone();
        other["version"] = json!(2);
        other.to_string()
    };
    for (name, damage) in [("corrupt", None), ("version-2", Some(foreign as fn(&Value) -> String))] {
        let mut s = Session::new();
        let (_, bg, _) = button_and_card(&mut s);
        let dir = Folder::new(&format!("manifest-{name}"));
        export_to(&mut s, &dir, json!({}));
        let text = damage.map_or_else(|| "{ \"version\": 1, \"files\": [".to_string(), |f| f(&manifest_of(&dir)));
        dir.put(MANIFEST, text.as_bytes());
        recolor(&mut s, bg, "#0000ff");
        let before = dir.snapshot();

        let v = export_to(&mut s, &dir, json!({}));
        let after = dir.snapshot();
        for path in GENERATED {
            assert_eq!(after[path], before[path], "{name}: {path} was overwritten without force");
        }
        assert!(written_but_the_manifest(&v).is_empty(), "{name}: {v}");
        assert!(skipped(&v).contains(&(BUTTON.to_string(), "not-generated".to_string())), "{name}: the output says so: {v}");

        export_to(&mut s, &dir, json!({"force": true}));
        assert!(String::from_utf8(dir.read(BUTTON)).unwrap().to_lowercase().contains("0000ff"), "{name}: force replaces");
    }
}

#[test]
fn a_folder_outside_the_write_roots_is_refused_and_one_inside_is_written() {
    // covers: E6-6.9
    let mut s = Session::new();
    button_and_card(&mut s);
    let base = Folder::new("roots");
    std::fs::create_dir_all(base.join("inside")).unwrap();
    std::fs::create_dir_all(base.join("outside")).unwrap();
    let roots = Arc::new(Roots::new(&[], &[base.join("inside").to_string_lossy().into_owned()]).unwrap());

    let refused = confined(Some(&roots), || s.execute("export.lit", &json!({"path": base.join("outside").to_string_lossy()})));
    let message = refused.expect_err("outside the write roots").to_string();
    assert!(message.contains("automation path rejected"), "{message}");
    assert!(base.snapshot().is_empty(), "nothing was written anywhere");

    let target = base.join("inside/package").to_string_lossy().into_owned();
    let written = confined(Some(&roots), || s.execute("export.lit", &json!({"path": target}))).unwrap();
    assert!(list(&written, "written").iter().any(|p| p == BUTTON), "{written}");
    assert!(base.join("inside/package").join(BUTTON).is_file());
}

#[test]
fn a_package_that_is_not_ready_writes_nothing_not_even_the_manifest() {
    // covers: E6-6.10
    let mut s = Session::new();
    button_and_card(&mut s);
    let fresh = Folder::new("not-ready-fresh");
    component(&mut s, "###", 400.0);
    match s.execute("export.lit", &json!({"path": fresh.path()})) {
        Err(EngineError::BadParams { cmd, msg }) => assert!(cmd == "export.lit" && !msg.is_empty()),
        other => panic!("expected a bad-parameters refusal, got {other:?}"),
    }
    assert!(fresh.snapshot().is_empty() && !fresh.join(".uigen").exists(), "an empty folder stays empty");

    let mut s = Session::new();
    let (_, bg, _) = button_and_card(&mut s);
    let used = Folder::new("not-ready-used");
    export_to(&mut s, &used, json!({}));
    let before = used.snapshot();
    component(&mut s, "###", 400.0);
    recolor(&mut s, bg, "#0000ff");
    assert!(matches!(s.execute("export.lit", &json!({"path": used.path()})), Err(EngineError::BadParams { .. })));
    assert_eq!(used.snapshot(), before, "files and manifest are as they were");
}
