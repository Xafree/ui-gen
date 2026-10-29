//! Command-line parsing: files, the control port and invite links.

use crate::args::{Args, control_port, file_url_path, is_invite, parse, require_roots_for_control};

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(ToString::to_string).collect()
}

#[test]
fn files_and_the_control_port() {
    assert_eq!(parse(&args(&[])), Args::default());
    let a = parse(&args(&["--control", "4242", "a.figcraft", "b.svg"]));
    assert_eq!(a, Args { control: Some(4242), files: args(&["a.figcraft", "b.svg"]), ..Args::default() });
    // A missing or bad port is ignored, not taken as a file.
    assert_eq!(parse(&args(&["--control"])), Args::default());
    assert_eq!(parse(&args(&["--control", "nope"])).control, None);
}

#[test]
fn the_control_port_accepts_the_equals_form() {
    let a = parse(&args(&["--control=7980", "a.figcraft"]));
    assert_eq!(a, Args { control: Some(7980), files: args(&["a.figcraft"]), ..Args::default() });
    // A bad port is ignored, not taken as a file.
    assert_eq!(parse(&args(&["--control=nope"])), Args::default());
}

#[test]
fn automation_roots_repeat_and_are_never_files() {
    let a = parse(&args(&["--automation-read-root", "in", "--automation-write-root=out", "--automation-read-root=more", "a.figcraft"]));
    assert_eq!(a.read_roots, args(&["in", "more"]));
    assert_eq!(a.write_roots, args(&["out"]));
    assert_eq!(a.files, args(&["a.figcraft"]));
    // A flag without its folder is kept as an empty one, which startup refuses.
    assert_eq!(parse(&args(&["--automation-write-root"])).write_roots, args(&[""]));
}

#[test]
fn unknown_flags_are_refused_never_ignored() {
    // #836: a mistyped root flag (or one an older release lacks) must stop the app, not start it
    // unconfined. Both forms, with and without `=`.
    let a = parse(&args(&["--automation-write-roots", "out", "--automation-write-dir=out", "--verbose", "a.figcraft"]));
    assert_eq!(a.unknown, args(&["--automation-write-roots", "--automation-write-dir=out", "--verbose"]));
    assert!(a.write_roots.is_empty());
    // Single-dash arguments are left as they were (files); after `--` everything is a file.
    let a = parse(&args(&["--control", "1", "--", "--odd.figcraft", "--automation-write-root=x"]));
    assert_eq!(a.files, args(&["--odd.figcraft", "--automation-write-root=x"]));
    assert!(a.unknown.is_empty() && a.write_roots.is_empty());
}

#[test]
fn invite_links_join_instead_of_opening() {
    let link = "figcraft://join?host=192.168.1.20:47900&file=abc&token=AbCdEfGhIjKlMnOpQrStUv";
    let a = parse(&args(&[link]));
    assert_eq!(a.join.as_deref(), Some(link));
    assert!(a.files.is_empty(), "a link is never opened as a file");
    // How browsers and shells pass it: any case, a slash after `join`, alongside files.
    let a = parse(&args(&["design.figcraft", "FIGCRAFT://join/?host=h:1&file=f&token=t"]));
    assert_eq!(a.files, args(&["design.figcraft"]));
    assert_eq!(a.join.as_deref(), Some("FIGCRAFT://join/?host=h:1&file=f&token=t"));
    assert!(is_invite("figcraft:join") && !is_invite("figcraft.svg") && !is_invite("fig") && !is_invite("ü€figcraft:"));
}

#[test]
fn file_urls_are_paths() {
    // What a Linux desktop passes for `%U`.
    let a = parse(&args(&["file:///home/ada/My%20Designs/a+b.figcraft", "FILE://localhost/tmp/x.svg"]));
    assert_eq!(a.files, args(&["/home/ada/My Designs/a+b.figcraft", "/tmp/x.svg"]), "spaces decoded, plus signs kept");
    assert_eq!(file_url_path("file:///%E2%82%AC.png").as_deref(), Some("/€.png"));
    let win = file_url_path("file:///C:/Designs/a.figcraft");
    assert_eq!(win.as_deref(), Some(if cfg!(windows) { "C:/Designs/a.figcraft" } else { "/C:/Designs/a.figcraft" }));
    for bad in ["file://server/share/a.svg", "file:///broken%2", "file:///bad%zz", "file:///%FF", "file:", "fi", "ü€file:///x"] {
        assert_eq!(file_url_path(bad), None, "{bad}");
    }
}

// covers: K1
#[test]
fn the_control_port_comes_from_the_flag_then_the_environment() {
    assert_eq!(control_port(&parse(&args(&["--control", "1"])), Some("2")), Some(1), "the flag wins");
    assert_eq!(control_port(&parse(&args(&[])), Some("7980")), Some(7980));
    assert_eq!(control_port(&parse(&args(&[])), Some("nope")), None);
    assert_eq!(control_port(&parse(&args(&[])), None), None);
}

// covers: K1
#[test]
fn the_control_channel_refuses_to_start_without_file_roots() {
    for given in [args(&["--control", "4242"]), args(&["--control=4242", "a.figcraft"])] {
        let a = parse(&given);
        let message = require_roots_for_control(control_port(&a, None), &a).unwrap_err();
        for expected in ["--control", "--automation-read-root", "--automation-write-root"] {
            assert!(message.contains(expected), "{expected} missing from: {message}");
        }
    }
    // The port from FIGCRAFT_CONTROL_PORT is the same control channel.
    let a = parse(&args(&["a.figcraft"]));
    assert!(require_roots_for_control(control_port(&a, Some("4242")), &a).is_err());
}

// covers: K1
#[test]
fn the_control_channel_starts_once_it_has_a_root() {
    for given in [
        args(&["--control", "4242", "--automation-read-root", "in", "--automation-write-root", "out"]),
        args(&["--control", "4242", "--automation-read-root=in"]),
        args(&["--control", "4242", "--automation-write-root=out"]),
    ] {
        let a = parse(&given);
        assert_eq!(require_roots_for_control(control_port(&a, None), &a), Ok(()), "{given:?}");
    }
    let a = parse(&args(&["--automation-write-root=out"]));
    assert_eq!(require_roots_for_control(control_port(&a, Some("4242")), &a), Ok(()), "roots with the port from the environment");
}

// covers: K1
#[test]
fn without_a_control_port_no_roots_are_required() {
    let a = parse(&args(&["a.figcraft"]));
    assert_eq!(require_roots_for_control(control_port(&a, None), &a), Ok(()));
}
