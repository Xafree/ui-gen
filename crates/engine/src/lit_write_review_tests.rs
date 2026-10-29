//! Review fixes of the Lit package write: a blank folder is refused, links inside the folder are
//! never written through or followed, the walk of the files the export did not generate leaves
//! `dist/` alone, says when it stopped early, and on Windows compares names without their case.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::components::tests::{ex, idv};
use crate::file_access::{Roots, confined};
use crate::{EngineError, Session};
use serde_json::{Value, json};

const BUTTON: &str = "components/ui-button.ts";

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-lit-review-{name}-{}", std::process::id()));
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

    fn put(&self, rel: &str, bytes: &[u8]) {
        let path = self.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn library() -> Session {
    let mut s = Session::new();
    let f = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 40, "name": "Button"})));
    ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": {"name": "Bg", "fill": "#ff0000"}}));
    ex(&mut s, "component.create", json!({"ids": [f]}));
    s
}

fn list(v: &Value, key: &str) -> Vec<String> {
    v.get(key).and_then(Value::as_array).map(|all| all.iter().filter_map(|p| p.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

#[cfg(unix)]
fn link_dir(target: &Path, at: &Path) -> bool {
    std::os::unix::fs::symlink(target, at).is_ok()
}

#[cfg(windows)]
fn link_dir(target: &Path, at: &Path) -> bool {
    std::os::windows::fs::symlink_dir(target, at).is_ok()
        || std::process::Command::new("cmd").arg("/C").arg("mklink").arg("/J").arg(at).arg(target).output().is_ok_and(|out| out.status.success())
}

#[cfg(unix)]
fn link_file(target: &Path, at: &Path) -> bool {
    std::os::unix::fs::symlink(target, at).is_ok()
}

#[cfg(windows)]
fn link_file(target: &Path, at: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, at).is_ok()
}

#[test]
fn a_blank_folder_is_a_bad_parameter_and_writes_nothing() {
    let mut s = library();
    let home = Folder::new("blank");
    let roots = Arc::new(Roots::new(&[], &[home.path()]).unwrap());
    for blank in ["", "   ", " \u{a0}\n"] {
        let err = confined(Some(&roots), || s.execute("export.lit", &json!({"path": blank}))).expect_err("a blank folder is refused");
        assert!(matches!(&err, EngineError::BadParams { cmd, msg } if cmd == "export.lit" && msg.contains("path")), "{blank:?}: {err}");
    }
}

#[test]
fn a_folder_that_is_a_link_inside_the_target_is_never_written_through() {
    let mut s = library();
    let outside = Folder::new("link-out");
    let target = Folder::new("link-dir");
    if !link_dir(&outside.0, &target.join("components")) {
        return;
    }
    let err = s.execute("export.lit", &json!({"path": target.path()})).expect_err("a link in the way is refused");
    assert!(err.to_string().contains("link"), "{err}");
    assert!(std::fs::read_dir(&outside.0).unwrap().next().is_none(), "nothing went through the link");
    assert!(!target.join(".uigen/export.json").exists(), "nothing was written, the manifest included");
}

#[test]
fn a_file_that_is_a_link_is_never_written_through() {
    let mut s = library();
    let outside = Folder::new("file-out");
    outside.put("elsewhere.ts", b"mine\n");
    let target = Folder::new("link-file");
    std::fs::create_dir_all(target.join("components")).unwrap();
    if !link_file(&outside.join("elsewhere.ts"), &target.join(BUTTON)) {
        return;
    }
    let err = s.execute("export.lit", &json!({"path": target.path(), "force": true})).expect_err("a link in the way is refused");
    assert!(err.to_string().contains("link"), "{err}");
    assert_eq!(std::fs::read(outside.join("elsewhere.ts")).unwrap(), b"mine\n");
}

#[test]
fn the_walk_of_kept_files_does_not_follow_a_link_to_a_folder() {
    let mut s = library();
    let outside = Folder::new("walk-out");
    outside.put("secret.txt", b"x");
    let target = Folder::new("walk-link");
    target.put("mine.txt", b"x");
    if !link_dir(&outside.0, &target.join("linked")) {
        return;
    }
    let v = s.execute("export.lit", &json!({"path": target.path()})).unwrap();
    let kept = list(&v, "kept");
    assert!(kept.contains(&"mine.txt".to_string()), "{kept:?}");
    assert!(kept.iter().all(|p| !p.contains("secret.txt")), "the link was followed: {kept:?}");
}

#[test]
fn the_walk_of_kept_files_leaves_dist_alone() {
    let mut s = library();
    let target = Folder::new("dist");
    target.put("dist/bundle.js", b"x");
    target.put("src/app.ts", b"x");
    let v = s.execute("export.lit", &json!({"path": target.path()})).unwrap();
    let kept = list(&v, "kept");
    assert_eq!(kept, vec!["src/app.ts".to_string()]);
}

#[test]
fn a_folder_with_too_many_entries_says_the_list_of_kept_files_is_partial() {
    let mut s = library();
    let target = Folder::new("many");
    std::fs::create_dir_all(target.join("src")).unwrap();
    for i in 0..10_001 {
        std::fs::write(target.join(&format!("src/f{i}.txt")), b"").unwrap();
    }
    let v = s.execute("export.lit", &json!({"path": target.path()})).unwrap();
    let notes = v["notes"].as_array().unwrap();
    assert!(notes.iter().any(|n| n["code"] == "kept-truncated"), "{notes:?}");
}

#[test]
fn a_small_folder_gets_no_truncation_note() {
    let mut s = library();
    let target = Folder::new("few");
    target.put("src/app.ts", b"x");
    let v = s.execute("export.lit", &json!({"path": target.path()})).unwrap();
    assert!(v["notes"].as_array().unwrap().iter().all(|n| n["code"] != "kept-truncated"));
}

#[cfg(windows)]
#[test]
fn names_that_differ_only_by_case_are_one_file_on_windows() {
    let mut s = library();
    let target = Folder::new("case");
    target.put("Components/UI-Button.ts", b"// hand made\n");
    for _ in 0..2 {
        let v = s.execute("export.lit", &json!({"path": target.path()})).unwrap();
        let skipped: Vec<String> = v["skipped"].as_array().unwrap().iter().filter_map(|e| e["path"].as_str().map(str::to_string)).collect();
        assert_eq!(skipped, vec![BUTTON.to_string()], "{v}");
        assert!(list(&v, "kept").is_empty(), "the same file is not listed twice: {v}");
    }
}
