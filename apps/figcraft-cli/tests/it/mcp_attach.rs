//! `uigen-cli mcp --attach PORT` against the control channel's session token.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_uigen-cli"))
}

fn config_root(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("figcraft-cli-attach-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn app_config_dir(root: &std::path::Path) -> std::path::PathBuf {
    if cfg!(target_os = "macos") { root.join("Library/Application Support/FigCraft") } else { root.join("FigCraft") }
}

fn attach(port: u16, root: &std::path::Path, token_env: Option<&str>) -> Output {
    let mut cmd = bin();
    cmd.args(["mcp", "--attach", &port.to_string()]).env("APPDATA", root).env("HOME", root).env("XDG_CONFIG_HOME", root);
    match token_env {
        Some(token) => cmd.env("FIGCRAFT_CONTROL_TOKEN", token),
        None => cmd.env_remove("FIGCRAFT_CONTROL_TOKEN"),
    };
    cmd.stdin(Stdio::null()).output().unwrap()
}

// covers: K4
#[test]
fn attach_without_a_token_names_where_it_is_expected_and_never_connects() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let root = config_root("no-token");
    let out = attach(port, &root, None);
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains(&format!("control-token-{port}")), "the expected file is named: {err}");
    assert!(err.contains("FIGCRAFT_CONTROL_TOKEN"), "the variable is named: {err}");
    let connection = listener.accept();
    assert!(matches!(&connection, Err(e) if e.kind() == std::io::ErrorKind::WouldBlock), "no connection without a token: {connection:?}");
    std::fs::remove_dir_all(root).unwrap();
}

fn first_line_received(port_listener: TcpListener) -> std::thread::JoinHandle<Value> {
    std::thread::spawn(move || {
        let (stream, _) = port_listener.accept().unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        writeln!(reader.get_mut(), r#"{{"id":null,"ok":true,"result":"authenticated"}}"#).unwrap();
        // The app keeps the connection until the client leaves, as the real one does.
        let mut rest = String::new();
        assert_eq!(reader.read_line(&mut rest).unwrap(), 0, "nothing follows the token: {rest}");
        serde_json::from_str(&line).unwrap()
    })
}

// covers: K4
#[test]
fn attach_sends_the_token_of_the_environment_first() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = first_line_received(listener);
    let root = config_root("env-token");
    let out = attach(port, &root, Some("token-from-env"));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(app.join().unwrap(), json!({"auth": "token-from-env"}));
    std::fs::remove_dir_all(root).unwrap();
}

// covers: K4
#[test]
fn attach_reads_the_token_file_of_the_apps_configuration_folder() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let root = config_root("file-token");
    let dir = app_config_dir(&root);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("control-token-{port}")), "token-from-file\n").unwrap();
    let app = first_line_received(listener);
    let out = attach(port, &root, None);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(app.join().unwrap(), json!({"auth": "token-from-file"}));
    std::fs::remove_dir_all(root).unwrap();
}

// covers: K5
#[test]
fn the_help_says_the_app_needs_a_root_and_where_the_token_is() {
    let out = bin().arg("help").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    for expected in ["--control PORT --automation-read-root", "control-token-PORT", "FIGCRAFT_CONTROL_TOKEN"] {
        assert!(text.contains(expected), "{expected} missing from the help");
    }
}

// covers: K5
#[test]
fn the_documents_say_the_control_channel_needs_a_root_and_a_token() {
    let docs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for file in ["docs/mcp.md", "docs/cli.md", "README.md"] {
        let text = std::fs::read_to_string(docs.join(file)).unwrap();
        for expected in ["--automation-read-root", "FIGCRAFT_CONTROL_TOKEN", "control-token-"] {
            assert!(text.contains(expected), "{expected} missing from {file}");
        }
    }
}
