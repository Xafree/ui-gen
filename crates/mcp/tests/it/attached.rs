//! The attached backend against a fake app: a loopback control server answering with a headless
//! session, speaking the desktop app's JSON-lines protocol.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;

use figcraft_engine::control_token::control_token_path;
use figcraft_mcp::{Attached, Backend, Headless, Server, control_token_from};
use serde_json::{Value, json};

const TOKEN: &str = "5ec2e7e6c1f84a0e9d3b7a6f1c0d8e42";

fn accept_authenticated(listener: &TcpListener) -> (TcpStream, BufReader<TcpStream>) {
    let (stream, _) = listener.accept().unwrap();
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    reader.read_line(&mut first).unwrap();
    let first: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(first, json!({"auth": TOKEN}), "the token is the first line of a connection");
    (stream, reader)
}

fn fake_app() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut h = Headless::new();
        for stream in listener.incoming().flatten() {
            let mut out = stream.try_clone().unwrap();
            let mut lines = BufReader::new(stream).lines();
            let first: Value = lines.next().and_then(Result::ok).and_then(|l| serde_json::from_str(&l).ok()).unwrap_or(Value::Null);
            if first != json!({"auth": TOKEN}) {
                let _ = writeln!(
                    out,
                    "{}",
                    json!({"id": null, "ok": false, "error": "authentication failed: the first line must be {\"auth\": \"<token>\"}"})
                );
                continue;
            }
            for line in lines {
                let Ok(line) = line else { break };
                let v: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
                let method = v["method"].as_str().unwrap_or("");
                let reply = match h.call(method, v["params"].clone()) {
                    Ok(r) => json!({"id": v["id"], "ok": true, "result": r}),
                    Err(e) => json!({"id": v["id"], "ok": false, "error": e}),
                };
                if writeln!(out, "{reply}").is_err() {
                    break;
                }
            }
        }
    });
    port
}

fn call(s: &mut Server, tool: &str, args: Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": tool, "arguments": args}}).to_string();
    let r: Value = serde_json::from_str(&s.handle_line(&line).unwrap()).unwrap();
    r["result"].clone()
}

#[test]
fn attached_backend_forwards_everything() {
    let port = fake_app();
    let backend = Attached::port(port, TOKEN).unwrap();
    assert!(backend.describe().contains(&port.to_string()));
    let mut s = Server::new(Box::new(backend));
    let r = call(&mut s, "create_node", json!({"type": "frame", "width": 64, "height": 32, "props": {"fill": "#FF0000"}}));
    assert_eq!(r["isError"], false, "{r}");
    let created: Value = serde_json::from_str(r["content"][0]["text"].as_str().unwrap()).unwrap();
    let id = created["id"].clone();
    // Render through ui.render + a temp file.
    let shot = call(&mut s, "screenshot", json!({}));
    assert_eq!(shot["content"][0]["type"], "image", "{shot}");
    let shot = call(&mut s, "screenshot", json!({"ids": [id], "scale": 2}));
    assert_eq!(shot["isError"], false, "{shot}");
    // Window capture through ui.screenshot.
    let win = call(&mut s, "screenshot", json!({"window": true}));
    assert_eq!(win["content"][0]["type"], "image", "{win}");
    let e = call(&mut s, "export", json!({"ids": [id]}));
    assert_eq!(e["content"][0]["mimeType"], "image/png");
    assert!(call(&mut s, "key", json!({"key": "Delete"}))["isError"] == false);
    let doc = call(&mut s, "inspect_document", json!({}));
    assert!(!doc["content"][0]["text"].as_str().unwrap().contains("\"Frame"));
    let err = call(&mut s, "run_command", json!({"command": "nope"}));
    assert_eq!(err["isError"], true);
}

#[test]
fn attached_to_nothing_fails_cleanly() {
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    assert!(Attached::port(port, TOKEN).is_err());
}

#[test]
fn lost_mutation_reply_is_not_replayed_and_next_call_reconnects() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = std::thread::spawn(move || {
        let mut h = Headless::new();
        let mut methods = Vec::new();
        for lose_reply in [true, false] {
            let (mut stream, mut reader) = accept_authenticated(&listener);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            let method = request["method"].as_str().unwrap();
            methods.push(method.to_string());
            let result = h.call(method, request["params"].clone()).unwrap();
            if !lose_reply {
                writeln!(stream, "{}", json!({"id": request["id"], "ok": true, "result": result})).unwrap();
            }
        }
        methods
    });
    let mut backend = Attached::port(port, TOKEN).unwrap();
    let result = backend.call("engine.execute", json!({"command": "shape.rectangle", "params": {"width": 64, "height": 32}}));
    assert!(result.is_err(), "a lost reply must not turn into a successful replay: {result:?}");
    let error = result.unwrap_err();
    assert!(error.contains("may have completed"), "{error}");
    assert!(error.contains("before retrying"), "{error}");
    let doc = backend.call("document.inspect", json!({"depth": 1})).unwrap();
    assert_eq!(doc["pages"][0]["children"].as_array().unwrap().len(), 1, "{doc}");
    assert_eq!(app.join().unwrap(), ["engine.execute", "document.inspect"]);
}

#[test]
fn a_connection_the_app_closed_while_idle_is_replaced_before_sending() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (closed_tx, closed_rx) = std::sync::mpsc::channel();
    let app = std::thread::spawn(move || {
        let mut h = Headless::new();
        let mut methods = Vec::new();
        // Each connection answers one request, then the app drops it as it does an idle one.
        for _ in 0..2 {
            let (mut stream, mut reader) = accept_authenticated(&listener);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            let method = request["method"].as_str().unwrap();
            methods.push(method.to_string());
            let result = h.call(method, request["params"].clone()).unwrap();
            writeln!(stream, "{}", json!({"id": request["id"], "ok": true, "result": result})).unwrap();
            drop(stream);
            closed_tx.send(()).unwrap();
        }
        methods
    });
    let mut backend = Attached::port(port, TOKEN).unwrap();
    backend.call("engine.execute", json!({"command": "shape.rectangle", "params": {"width": 64, "height": 32}})).unwrap();
    closed_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    // No error, nothing sent twice: the request goes out once, on a fresh connection.
    let doc = backend.call("document.inspect", json!({"depth": 1})).unwrap();
    assert_eq!(doc["pages"][0]["children"].as_array().unwrap().len(), 1, "{doc}");
    assert_eq!(app.join().unwrap(), ["engine.execute", "document.inspect"]);
}

#[test]
fn reconnect_failure_reports_request_not_sent() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut backend = Attached::port(listener.local_addr().unwrap().port(), TOKEN).unwrap();
    drop(listener);
    assert!(backend.call("document.inspect", json!({})).is_err());
    let error = backend.call("document.inspect", json!({})).unwrap_err();
    assert!(error.contains("request was not sent"), "{error}");
    assert!(error.contains("--automation-read-root"), "the hint to start the app names the root it needs: {error}");
}

#[test]
fn a_wrong_token_is_refused_by_the_app_and_reported() {
    let port = fake_app();
    let error = Attached::port(port, "not-the-token").err().unwrap();
    assert!(error.to_string().contains("authentication failed"), "{error}");
}

#[test]
fn every_new_connection_starts_with_the_token() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = std::thread::spawn(move || {
        let mut h = Headless::new();
        for _ in 0..2 {
            let (mut stream, mut reader) = accept_authenticated(&listener);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            let result = h.call(request["method"].as_str().unwrap(), request["params"].clone()).unwrap();
            writeln!(stream, "{}", json!({"id": request["id"], "ok": true, "result": result})).unwrap();
        }
    });
    let mut backend = Attached::port(port, TOKEN).unwrap();
    backend.call("document.inspect", json!({"depth": 1})).unwrap();
    // The app dropped the first connection; the replacement authenticates again.
    std::thread::sleep(std::time::Duration::from_millis(100));
    backend.call("document.inspect", json!({"depth": 1})).unwrap();
    app.join().unwrap();
}

fn config_with_token(name: &str, port: u16, contents: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("figcraft-mcp-token-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(control_token_path(&dir, port), contents).unwrap();
    dir
}

#[test]
fn the_token_comes_from_the_environment_first() {
    let dir = config_with_token("env-first", 7979, "from-the-file");
    assert_eq!(control_token_from(Some("from-the-env"), Some(&dir), 7979).unwrap(), "from-the-env");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_token_otherwise_comes_from_the_apps_file_for_that_port() {
    let dir = config_with_token("file", 7979, "from-the-file\n");
    assert_eq!(control_token_from(None, Some(&dir), 7979).unwrap(), "from-the-file", "the final newline is not part of the token");
    assert_eq!(control_token_from(Some(""), Some(&dir), 7979).unwrap(), "from-the-file", "an empty variable is no token");
    let other_port = control_token_from(None, Some(&dir), 7980).unwrap_err();
    assert!(other_port.contains("control-token-7980"), "{other_port}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn without_a_token_the_error_says_where_it_is_expected() {
    let dir = std::env::temp_dir().join(format!("figcraft-mcp-token-{}-missing", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let error = control_token_from(None, Some(&dir), 7979).unwrap_err();
    assert!(error.contains(&control_token_path(&dir, 7979).display().to_string()), "{error}");
    assert!(error.contains("FIGCRAFT_CONTROL_TOKEN"), "{error}");
    let empty = config_with_token("empty", 7979, "  \n");
    assert!(control_token_from(None, Some(&empty), 7979).unwrap_err().contains("control-token-7979"));
    let no_dir = control_token_from(None, None, 7979).unwrap_err();
    assert!(no_dir.contains("FIGCRAFT_CONTROL_TOKEN"), "{no_dir}");
    std::fs::remove_dir_all(dir).unwrap();
    std::fs::remove_dir_all(empty).unwrap();
}
