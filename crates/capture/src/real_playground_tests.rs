//! The playground end to end: the real capture service (Node and the Chromium of this machine, no
//! network) behind a real engine session, on a temporary library project with synthetic components.
//! Every test is `#[ignore]`d because it drives Chromium: run them by hand with
//! `cargo test --offline -p figcraft-capture real_playground -- --ignored --test-threads=1 --nocapture`.

use std::fs;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use figcraft_engine::Session;
use figcraft_engine::capture::{CaptureEvent, CaptureOutcome};
use figcraft_engine::playground::{Frame, FrameFormat, Status};
use image::RgbImage;
use serde_json::{Value, json};

use crate::service_tests::TempDir;
use crate::{CaptureService, locate};

const OPEN: &str = "playground.open";
const OPEN_LIMIT: Duration = Duration::from_secs(120);
const FRAME_LIMIT: Duration = Duration::from_secs(10);
const SETTLE_QUIET: Duration = Duration::from_millis(500);
const GONE_LIMIT: Duration = Duration::from_secs(15);
const CHANGED_BY: u8 = 16;
const LATENCY_BUDGET_MS: f64 = 150.0;
const SETTINGS_MEASURED: u32 = 20;
const OBSERVERS: [&str; 3] = ["powershell", "conhost", "ps"];
const JPEG_START: [u8; 2] = [0xff, 0xd8];

#[derive(Clone)]
struct Shot {
    seq: u64,
    format: FrameFormat,
    bytes: usize,
    jpeg_start: bool,
    pixels: RgbImage,
}

impl Shot {
    fn of(frame: &Frame) -> Self {
        let pixels = image::load_from_memory(&frame.data).expect("the frame is a decodable image").to_rgb8();
        Self { seq: frame.seq, format: frame.format, bytes: frame.data.len(), jpeg_start: frame.data.starts_with(&JPEG_START), pixels }
    }

    fn corner_brightness(&self) -> u32 {
        let [r, g, b] = self.pixels.get_pixel(2, 2).0;
        (u32::from(r) + u32::from(g) + u32::from(b)) / 3
    }

    fn changed_pixels_since(&self, other: &Shot) -> usize {
        if self.pixels.dimensions() != other.pixels.dimensions() {
            return self.pixels.pixels().count();
        }
        self.pixels
            .pixels()
            .zip(other.pixels.pixels())
            .filter(|(now, before)| now.0.iter().zip(before.0.iter()).any(|(a, b)| a.abs_diff(*b) >= CHANGED_BY))
            .count()
    }
}

struct Bench {
    session: Session,
    dir: TempDir,
    events: Vec<CaptureEvent>,
    last: Option<Shot>,
}

fn command(session: &mut Session, name: &str, params: Value) -> Value {
    session.execute(name, &params).unwrap_or_else(|error| panic!("{name} {params}: {error}"))
}

fn component(session: &mut Session, name: &str, x: usize, fill: &str) {
    let frame = command(session, "frame.create", json!({"x": x, "y": 0, "width": 120, "height": 48, "name": name}))["id"].clone();
    command(session, "layout.add", json!({"ids": [frame]}));
    let background = command(
        session,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 120, "height": 48, "parent": frame, "props": {"name": "Background", "fill": "#ff0000"}}),
    )["id"]
        .clone();
    command(session, "variable.bind", json!({"ids": [background], "field": "fill", "variable": fill}));
    command(session, "text.create", json!({"text": name, "x": 12, "y": 12, "parent": frame, "props": {"name": "Label"}}));
    command(session, "component.create", json!({"ids": [frame]}));
}

fn tokens(session: &mut Session) {
    command(session, "variable.createCollection", json!({"name": "Theme", "modes": ["Light", "Dark"]}));
    for (name, light, dark) in [
        ("color/primary", "#e5532d", "#ff7a59"),
        ("color/surface", "#f5f5f5", "#1a1a1a"),
        ("color/border", "#d8d3ce", "#333333"),
        ("color/success", "#22aa55", "#33cc77"),
    ] {
        command(session, "variable.create", json!({"collection": "Theme", "name": name, "type": "color", "values": {"Light": light, "Dark": dark}}));
    }
}

fn library(session: &mut Session, names: &[&str]) {
    let fills = ["color/primary", "color/surface", "color/border", "color/success"];
    for (index, name) in names.iter().enumerate() {
        component(session, name, index * 160, fills[index % fills.len()]);
    }
}

fn descendants_of_this_test() -> Option<Vec<String>> {
    let listing = if cfg!(windows) {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Get-CimInstance Win32_Process | ForEach-Object { \"$($_.ProcessId) $($_.ParentProcessId) $($_.Name)\" }",
            ])
            .output()
    } else {
        Command::new("ps").args(["-eo", "pid=,ppid=,comm="]).output()
    }
    .ok()?;
    let text = String::from_utf8_lossy(&listing.stdout).into_owned();
    let rows: Vec<(u32, u32, String)> = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.collect::<Vec<_>>().join(" ")))
        })
        .collect();
    let mut family = vec![std::process::id()];
    let mut names = Vec::new();
    let mut next = 0;
    while let Some(parent) = family.get(next).copied() {
        for (pid, ppid, name) in &rows {
            if *ppid == parent && !family.contains(pid) {
                family.push(*pid);
                if !OBSERVERS.iter().any(|observer| name.starts_with(observer)) {
                    names.push(format!("{name} ({pid})"));
                }
            }
        }
        next += 1;
    }
    Some(names)
}

impl Bench {
    fn new(label: &str, names: &[&str]) -> Self {
        let dir = TempDir::new(label);
        let config = locate().expect("node and the sidecar script must be found (UIGEN_NODE, UIGEN_SIDECAR_DIR)");
        let mut session = Session::empty();
        session.set_capture_backend(Box::new(CaptureService::start(config, Box::new(|| {}))));
        let opened = command(&mut session, "project.open", json!({"path": dir.path().display().to_string(), "create": true}));
        assert_eq!(opened["opened"], json!(true), "{opened}");
        tokens(&mut session);
        library(&mut session, names);
        Self { session, dir, events: Vec::new(), last: None }
    }

    fn pump(&mut self) {
        let events = self.session.pump_capture();
        self.events.extend(events);
    }

    fn event_of(&mut self, name: &str, job: u64) -> CaptureOutcome {
        let deadline = Instant::now() + OPEN_LIMIT;
        loop {
            self.pump();
            if let Some(found) = self.events.iter().find(|event| event.command == name && event.job == job) {
                return found.outcome.clone();
            }
            assert!(Instant::now() < deadline, "no {name} event within {OPEN_LIMIT:?}");
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn open(&mut self, params: Value) -> CaptureOutcome {
        let started = command(&mut self.session, OPEN, params);
        let job = started["job"].as_u64().unwrap_or_else(|| panic!("no job id in {started}"));
        let outcome = self.event_of(OPEN, job);
        self.last = None;
        outcome
    }

    fn refresh(&mut self) -> CaptureOutcome {
        let started = command(&mut self.session, "playground.refresh", json!({}));
        let job = started["job"].as_u64().unwrap_or_else(|| panic!("no job id in {started}"));
        let outcome = self.event_of("playground.refresh", job);
        self.last = None;
        outcome
    }

    fn open_events(&self) -> usize {
        self.events.iter().filter(|event| event.command == OPEN).count()
    }

    fn next_frame(&mut self, deadline: Instant) -> Option<Shot> {
        loop {
            self.session.playground_poll();
            self.pump();
            if let Some(frame) = self.session.playground_take_frame() {
                return Some(Shot::of(&frame));
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn first_frame_where(&mut self, accept: impl Fn(&Shot, Option<&Shot>) -> bool) -> Option<Shot> {
        let deadline = Instant::now() + FRAME_LIMIT;
        while let Some(shot) = self.next_frame(deadline) {
            let accepted = accept(&shot, self.last.as_ref());
            self.last = Some(shot.clone());
            if accepted {
                return Some(shot);
            }
        }
        None
    }

    fn settle(&mut self) -> Shot {
        self.first_frame_where(|_, _| true).expect("the page sent no image");
        let mut quiet_since = Instant::now();
        while quiet_since.elapsed() < SETTLE_QUIET {
            let Some(shot) = self.next_frame(Instant::now() + SETTLE_QUIET) else { break };
            if self.last.as_ref().is_some_and(|last| shot.changed_pixels_since(last) > 0) {
                quiet_since = Instant::now();
            }
            self.last = Some(shot);
        }
        self.last.clone().expect("an image is known")
    }

    fn change(&mut self, command_name: &str, params: Value) -> Instant {
        let started = Instant::now();
        command(&mut self.session, command_name, params);
        started
    }

    fn state(&mut self) -> Value {
        command(&mut self.session, "playground.state", json!({}))
    }
}

impl Bench {
    fn shows_change_since(&mut self, reference: &Shot) -> Option<(Shot, usize)> {
        let shot = self.first_frame_where(|shot, _| shot.changed_pixels_since(reference) > 0)?;
        let changed = shot.changed_pixels_since(reference);
        Some((shot, changed))
    }
}

fn hue_hex(step: u32) -> String {
    let hue = f64::from(step * 18 + 7) / 60.0;
    let (saturation, value) = (0.7_f64, 0.8_f64);
    let chroma = value * saturation;
    let x = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let (r, g, b) = match hue as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let shift = value - chroma;
    let channel = |part: f64| ((part + shift) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", channel(r), channel(g), channel(b))
}

fn ready(bench: &Bench) -> bool {
    matches!(bench.session.playground_status(), Status::Ready)
}

fn names(value: &Value, key: &str) -> Vec<String> {
    value[key].as_array().map(|all| all.iter().filter_map(|item| item.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

// covers: E7-8.1
#[test]
#[ignore = "drives Chromium; run by hand"]
fn real_playground_opens_pushes_an_accent_without_reloading_darkens_in_dark_mode_and_leaves_no_process() {
    let mut bench = Bench::new("real-playground", &["Button", "Card", "Badge"]);
    let before = descendants_of_this_test();
    eprintln!("processes below the test before the open: {before:?}");

    let opened =
        bench.open(json!({"template": "components", "width": 900, "height": 600})).unwrap_or_else(|error| panic!("open failed: {}", error.message));
    eprintln!("open: {opened}");
    assert!(ready(&bench), "the playground is not ready after a successful open");
    assert_eq!(names(&opened, "missing"), Vec::<String>::new(), "{opened}");
    assert_eq!(names(&opened, "defined").len(), 3, "{opened}");

    let baseline = bench.settle();
    eprintln!(
        "first image: seq {} {:?} {} bytes, {}x{}",
        baseline.seq,
        baseline.format,
        baseline.bytes,
        baseline.pixels.width(),
        baseline.pixels.height()
    );
    assert_eq!(baseline.format, FrameFormat::Jpeg);
    assert!(baseline.jpeg_start && baseline.bytes > 0, "the first image is not a JPEG");
    let light_corner = baseline.corner_brightness();

    bench.change("playground.set", json!({"accent": "#2e7d32"}));
    let (accented, changed) = bench.shows_change_since(&baseline).expect("no image shows the new accent");
    eprintln!("accent #2e7d32: {changed} pixels changed at seq {}", accented.seq);
    assert!(changed >= 1);
    assert_eq!(bench.open_events(), 1, "the accent change opened the page again");

    let light_before_dark = accented.corner_brightness();
    bench.change("playground.set", json!({"mode": "dark"}));
    let darker = bench.first_frame_where(|shot, _| shot.corner_brightness() + 40 < light_before_dark).expect("the background did not get darker");
    eprintln!("background brightness: light {light_corner}, dark {}", darker.corner_brightness());
    assert_eq!(bench.open_events(), 1, "the mode change opened the page again");

    command(&mut bench.session, "playground.close", json!({}));
    assert_eq!(bench.state()["status"], json!("closed"));
    thread::sleep(Duration::from_millis(500));
    bench.pump();
    eprintln!("processes below the test after playground.close: {:?}", descendants_of_this_test());

    drop(bench);
    let deadline = Instant::now() + GONE_LIMIT;
    loop {
        let left = descendants_of_this_test();
        eprintln!("processes below the test after the session is dropped: {left:?}");
        match left {
            Some(left) if left.is_empty() => break,
            None => break,
            Some(left) => assert!(Instant::now() < deadline, "processes remain after the service stops: {left:?}"),
        }
        thread::sleep(Duration::from_millis(500));
    }
}

// covers: E7-8.2
#[test]
#[ignore = "drives Chromium; run by hand"]
fn real_playground_measures_the_time_from_a_setting_to_the_image_that_shows_it_over_twenty_settings() {
    let mut bench = Bench::new("real-playground-latency", &["Button", "Card", "Badge"]);
    bench.open(json!({"template": "components", "width": 900, "height": 600})).unwrap_or_else(|error| panic!("open failed: {}", error.message));
    let mut shown = bench.settle();

    let mut millis: Vec<f64> = Vec::new();
    for step in 0..SETTINGS_MEASURED {
        let started = bench.change("playground.set", json!({"accent": hue_hex(step), "commit": false}));
        let found = bench.shows_change_since(&shown);
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        let Some((next, _)) = found else { panic!("no image showed setting {step} ({})", hue_hex(step)) };
        shown = next;
        millis.push(elapsed);
    }

    let mut sorted = millis.clone();
    sorted.sort_by(f64::total_cmp);
    let median = (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0;
    let (min, max) = (sorted[0], sorted[sorted.len() - 1]);
    eprintln!(
        "latency of playground.set to the image that shows it, {SETTINGS_MEASURED} settings: median {median:.0} ms, min {min:.0} ms, max {max:.0} ms"
    );
    eprintln!("all (ms): {:?}", millis.iter().map(|ms| ms.round()).collect::<Vec<_>>());
    if median > LATENCY_BUDGET_MS {
        eprintln!(
            "THE MEDIAN OF {median:.0} ms EXCEEDS THE {LATENCY_BUDGET_MS:.0} ms BUDGET: option B (separate Chromium window) is the fallback to decide"
        );
    } else {
        eprintln!("the median of {median:.0} ms is within the {LATENCY_BUDGET_MS:.0} ms budget");
    }
}

// covers: E7-8.3
#[test]
#[ignore = "drives Chromium; run by hand"]
fn real_playground_marks_a_missing_chart_in_the_model_and_the_image_shows_the_marker() {
    let mut bench = Bench::new("real-playground-missing", &["Button", "Badge", "Avatar"]);
    bench.open(json!({"template": "dashboard", "width": 1280, "height": 900})).unwrap_or_else(|error| panic!("open failed: {}", error.message));
    let state = bench.state();
    assert!(names(&state, "missing").contains(&"Chart".to_string()), "{state}");
    let staged = fs::read_to_string(bench.dir.path().join(".uigen/playground/template.html")).expect("the model is staged");
    assert!(staged.contains("data-uigen-missing=\"Chart\""), "no marker for Chart in the staged model");
    let with_marker = bench.settle();
    eprintln!("dashboard with Chart missing: {}x{}, missing {:?}", with_marker.pixels.width(), with_marker.pixels.height(), names(&state, "missing"));

    let session = &mut bench.session;
    component(session, "Chart", 640, "color/primary");
    bench.refresh().unwrap_or_else(|error| panic!("refresh failed: {}", error.message));
    let state = bench.state();
    assert!(!names(&state, "missing").contains(&"Chart".to_string()), "{state}");
    let without_marker = bench.settle();
    let differing = without_marker.changed_pixels_since(&with_marker);
    eprintln!("pixels that differ once Chart exists and its marker is gone: {differing}");
    assert!(differing > 0, "the image does not change when the Chart marker goes away: the marker is not visible");
}
