//! `uigen-cli mcp --attach HOST:PORT` for a host that is not this machine.

use std::process::Command;

fn attach(root: &std::path::Path, addr: &str, token_env: Option<&str>) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_uigen-cli"));
    cmd.args(["mcp", "--attach", addr]).env("APPDATA", root).env("HOME", root).env("XDG_CONFIG_HOME", root);
    match token_env {
        Some(token) => cmd.env("FIGCRAFT_CONTROL_TOKEN", token),
        None => cmd.env_remove("FIGCRAFT_CONTROL_TOKEN"),
    };
    cmd.stdin(std::process::Stdio::null()).output().unwrap()
}

// covers: K4
#[test]
fn attach_to_another_host_does_not_use_the_local_token_file() {
    let root = std::env::temp_dir().join(format!("figcraft-cli-attach-remote-{}", std::process::id()));
    let config = if cfg!(target_os = "macos") { root.join("Library/Application Support/FigCraft") } else { root.join("FigCraft") };
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("control-token-7979"), "local-file-token\n").unwrap();
    let out = attach(&root, "192.0.2.1:7979", None);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("FIGCRAFT_CONTROL_TOKEN"), "{err}");
    assert!(err.contains("never to another host"), "the file is not read for a remote host: {err}");
    assert!(!err.contains("can't reach"), "no connection is attempted without a token for that host: {err}");
    assert!(!err.contains("local-file-token"), "{err}");
    std::fs::remove_dir_all(root).unwrap();
}
