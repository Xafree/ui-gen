//! A service that died on its own can leave npm and Chromium behind: its tree is still cleaned.

use std::io;
use std::process::{Child, Command, Stdio};

use crate::process::{kill_tree, must_kill_tree};

fn exited_with(code: i32) -> io::Result<Child> {
    let (shell, flag) = if cfg!(windows) { ("cmd", "/C") } else { ("sh", "-c") };
    let mut child =
        Command::new(shell).args([flag, &format!("exit {code}")]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
    child.wait()?;
    Ok(child)
}

// covers: E2-5.5
#[test]
fn the_tree_of_a_service_that_crashed_or_still_runs_is_killed_but_not_that_of_a_clean_exit() -> io::Result<()> {
    let clean = exited_with(0)?.try_wait()?;
    let crashed = exited_with(3)?.try_wait()?;

    assert!(!must_kill_tree(clean), "a clean exit has already stopped its npm and Chromium");
    assert!(must_kill_tree(crashed));
    assert!(must_kill_tree(None));
    Ok(())
}

// covers: E2-5.5
#[test]
fn killing_the_tree_of_a_process_that_already_crashed_returns_without_panicking() -> io::Result<()> {
    let mut child = exited_with(3)?;
    kill_tree(&mut child);
    assert!(child.try_wait()?.is_some());
    Ok(())
}
