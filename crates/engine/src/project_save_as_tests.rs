//! `file.saveAs` of a project file into another folder: the tab belongs to the project of the folder
//! it now lives in, if that folder's manifest lists it, and is an ordinary file otherwise.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use crate::Session;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-project-save-as-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn folder(path: &str) -> PathBuf {
    std::fs::canonicalize(path).unwrap()
}

fn same_folder(a: &str, b: &Path) -> bool {
    folder(a) == std::fs::canonicalize(b).unwrap()
}

fn open_project(tag: &str) -> (Dir, Session) {
    let dir = Dir::new(tag);
    let mut s = Session::empty();
    let r = s.execute("project.open", &json!({"path": dir.0.to_string_lossy(), "create": true})).unwrap();
    assert_eq!(r["opened"], json!(true), "{r}");
    assert!(s.in_lib_project());
    (dir, s)
}

fn save_as(s: &mut Session, path: &str) -> Value {
    s.execute("file.saveAs", &json!({"path": path})).unwrap()
}

// covers: L1-E5
#[test]
fn saving_a_project_file_into_a_folder_without_a_manifest_makes_it_an_ordinary_file() {
    let (_dir, mut s) = open_project("plain");
    let elsewhere = Dir::new("plain-target");
    save_as(&mut s, &elsewhere.path("copy.figcraft"));
    assert!(!s.in_lib_project());
    let info = s.execute("project.info", &json!({})).unwrap();
    assert!(info["project"].is_null(), "{info}");
}

// covers: L1-E5
#[test]
fn saving_a_project_file_into_a_folder_that_lists_it_binds_it_to_that_folder() {
    let (_dir, mut s) = open_project("listed");
    let other = Dir::new("listed-target");
    std::fs::write(other.path("uigen.json"), r#"{"version":1,"name":"other","designFiles":["copy.figcraft"]}"#).unwrap();
    save_as(&mut s, &other.path("copy.figcraft"));
    let binding = s.project().expect("bound to the new folder");
    assert!(same_folder(&binding.root, &other.0), "{}", binding.root);
    assert_eq!(binding.config.name, "other");
}

// covers: L1-E5
#[test]
fn saving_a_project_file_into_a_folder_that_does_not_list_it_makes_it_an_ordinary_file() {
    let (_dir, mut s) = open_project("unlisted");
    let other = Dir::new("unlisted-target");
    std::fs::write(other.path("uigen.json"), r#"{"version":1,"name":"other","designFiles":["lib.figcraft"]}"#).unwrap();
    save_as(&mut s, &other.path("copy.figcraft"));
    assert!(!s.in_lib_project());
}

// covers: L1-E5
#[test]
fn saving_a_project_file_into_a_folder_with_an_unreadable_manifest_makes_it_ordinary_and_says_why() {
    let (_dir, mut s) = open_project("unreadable");
    let other = Dir::new("unreadable-target");
    std::fs::write(other.path("uigen.json"), "{not json").unwrap();
    save_as(&mut s, &other.path("copy.figcraft"));
    assert!(!s.in_lib_project());
    let message = s.message.take().expect("the session says why");
    assert!(message.starts_with("uigen.json: ") && message.ends_with("; saved as an ordinary file"), "{message}");
}

// covers: L1-E5
#[test]
fn saving_a_project_file_in_its_own_folder_keeps_it_in_the_project() {
    let (dir, mut s) = open_project("same-folder");
    save_as(&mut s, &dir.path("lib.figcraft"));
    let binding = s.project().expect("still bound");
    assert!(same_folder(&binding.root, &dir.0));
}
