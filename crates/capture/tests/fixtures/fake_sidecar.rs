//! A stand-in for `node service.mjs`, compiled by the tests with plain `rustc` (std only).
//!
//! Started as `fake-sidecar <scenario-file>`. The scenario file holds `key=value` lines:
//! - `dir`: folder for the marker files `launches`, `heartbeat` and `shutdown-received`;
//! - `hello`: `ok` (default), `v2` (greets with protocol 2), `absent` (exits without greeting),
//!   `silent` (stays alive and never greets);
//! - `delay_ms`: wait before every normal reply;
//! - `kinds` and `message`: the nth request is answered with the nth error kind of the list;
//! - `fault`: misbehaviour on the very first request of the very first launch only (`silent`,
//!   `die`, `notjson`, `toolong`); a relaunched fake always behaves;
//! - `shutdown`: `reply` (default) answers `shutdown` and exits, `ignore` keeps running until killed.
//!
//! A normal reply echoes the raw request line: `{"id":N,"ok":true,"result":{"request":<line>}}`.
//! Whatever happens, the fake ends itself after 30 seconds.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const SAFETY_LIMIT: Duration = Duration::from_secs(30);
const HEARTBEAT_PERIOD: Duration = Duration::from_millis(20);
const OVERSIZED_BYTES: usize = 200_000;

type Settings = HashMap<String, String>;

fn main() {
    let Some(scenario) = std::env::args().nth(1) else {
        std::process::exit(2);
    };
    let settings = read_settings(&scenario);
    let dir = PathBuf::from(setting(&settings, "dir", ""));
    let launch = register_launch(&dir);
    spawn_watchdog();
    spawn_heartbeat(dir.clone());
    if greet(setting(&settings, "hello", "ok")) {
        serve(&settings, &dir, launch);
    }
}

fn setting<'a>(settings: &'a Settings, key: &str, default: &'a str) -> &'a str {
    settings.get(key).map_or(default, String::as_str)
}

fn read_settings(path: &str) -> Settings {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn append(path: &Path, bytes: &[u8]) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(bytes);
    }
}

fn register_launch(dir: &Path) -> usize {
    let file = dir.join("launches");
    let before = fs::read_to_string(&file).map_or(0, |text| text.lines().count());
    append(&file, b"launch\n");
    before + 1
}

fn spawn_watchdog() {
    thread::spawn(|| {
        thread::sleep(SAFETY_LIMIT);
        std::process::exit(3);
    });
}

fn spawn_heartbeat(dir: PathBuf) {
    thread::spawn(move || loop {
        append(&dir.join("heartbeat"), b".");
        thread::sleep(HEARTBEAT_PERIOD);
    });
}

fn park() -> ! {
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

fn send(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(line.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

fn greet(mode: &str) -> bool {
    match mode {
        "absent" => return false,
        "silent" => park(),
        "v2" => send(r#"{"protocol":2,"service":"uigen-sidecar","node":"0.0.0-fake"}"#),
        _ => send(r#"{"protocol":1,"service":"uigen-sidecar","node":"0.0.0-fake"}"#),
    }
    true
}

fn serve(settings: &Settings, dir: &Path, launch: usize) {
    let kinds: Vec<&str> = setting(settings, "kinds", "").split(',').filter(|kind| !kind.is_empty()).collect();
    let mut served = 0usize;
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        served += 1;
        let id = top_level_value(&line, "id").unwrap_or_else(|| "0".to_string());
        let method = top_level_value(&line, "method").unwrap_or_default().trim_matches('"').to_string();
        if method == "shutdown" {
            append(&dir.join("shutdown-received"), b"shutdown\n");
            if setting(settings, "shutdown", "reply") == "ignore" {
                park();
            }
            send(&format!(r#"{{"id":{id},"ok":true,"result":{{}}}}"#));
            return;
        }
        let fault = if launch == 1 && served == 1 { setting(settings, "fault", "none") } else { "none" };
        match (kinds.get(served - 1), fault) {
            (_, "silent") => park(),
            (_, "die") => std::process::exit(1),
            (_, "notjson") => send("this is not json"),
            (_, "toolong") => send(&format!(r#"{{"id":{id},"ok":true,"result":{{"blob":"{}"}}}}"#, "x".repeat(OVERSIZED_BYTES))),
            (Some(kind), _) => {
                send(&format!(r#"{{"id":{id},"ok":false,"error":{{"kind":"{kind}","message":"{}"}}}}"#, setting(settings, "message", "failed")))
            }
            (None, _) => {
                let delay: u64 = setting(settings, "delay_ms", "0").parse().unwrap_or(0);
                thread::sleep(Duration::from_millis(delay));
                send(&format!(r#"{{"id":{id},"ok":true,"result":{{"request":{line}}}}}"#));
            }
        }
    }
    if setting(settings, "shutdown", "reply") == "ignore" {
        park();
    }
}

fn string_end(bytes: &[u8], open: usize) -> usize {
    let mut index = open + 1;
    while index < bytes.len() && bytes[index] != b'"' {
        index += if bytes[index] == b'\\' { 2 } else { 1 };
    }
    index.min(bytes.len())
}

fn skip_spaces(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

fn value_text(line: &str, start: usize) -> String {
    let bytes = line.as_bytes();
    let mut depth = 0i32;
    let mut index = start;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index = string_end(bytes, index) + 1;
                continue;
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' if depth == 0 => break,
            b'}' | b']' => depth -= 1,
            b',' if depth == 0 => break,
            _ => {}
        }
        index += 1;
    }
    line.get(start..index.min(line.len())).unwrap_or_default().trim().to_string()
}

fn top_level_value(line: &str, key: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut depth = 0i32;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth -= 1,
            b'"' => {
                let end = string_end(bytes, index);
                if depth == 1 && line.get(index + 1..end) == Some(key) {
                    let colon = skip_spaces(bytes, end + 1);
                    if bytes.get(colon) == Some(&b':') {
                        return Some(value_text(line, skip_spaces(bytes, colon + 1)));
                    }
                }
                index = end;
            }
            _ => {}
        }
        index += 1;
    }
    None
}
