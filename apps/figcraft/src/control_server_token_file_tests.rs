#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("figcraft-control-token-file-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn the_token_file_appears_by_rename_and_leaves_no_temporary_file() {
    let dir = scratch("rename");
    std::fs::write(dir.join("control-token-4243.tmp"), "left by a crashed run").unwrap();
    let path = write_token_file(&dir, 4243, "fresh-token").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "fresh-token");
    assert_eq!(files_in(&dir), ["control-token-4243"], "the temporary file is gone");
    let path = write_token_file(&dir, 4243, "newer-token").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "newer-token", "an existing file is replaced");
    assert_eq!(files_in(&dir), ["control-token-4243"]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn removing_the_token_file_tolerates_a_file_that_is_already_gone() {
    let dir = scratch("remove");
    let path = write_token_file(&dir, 4244, "token").unwrap();
    remove_token_file(&path);
    assert!(files_in(&dir).is_empty());
    remove_token_file(&path);
    std::fs::remove_dir_all(dir).unwrap();
}
