//! After `--`, an invite link is the only argument that counts.

use crate::args::parse;

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(ToString::to_string).collect()
}

#[test]
fn an_invite_after_the_separator_drops_every_other_argument() {
    let link = "figcraft://join?x";
    let unc = r"\h\s\a.figcraft";
    let a = parse(&args(&["--", link, unc]));
    assert_eq!(a.join.as_deref(), Some(link));
    assert!(a.files.is_empty(), "a file smuggled in next to the link is never opened");
    assert_eq!(a.control, None);
    assert_eq!(a.ignored, args(&[unc]));
}

#[test]
fn the_dropped_arguments_come_from_before_and_after_the_link() {
    let link = "figcraft://join?x";
    let a = parse(&args(&["--", "a.figcraft", link, "--control", "7979"]));
    assert_eq!(a.join.as_deref(), Some(link));
    assert!(a.files.is_empty());
    assert_eq!(a.control, None);
    assert_eq!(a.ignored, args(&["a.figcraft", "--control", "7979"]));
}

#[test]
fn without_an_invite_nothing_is_dropped_after_the_separator() {
    let a = parse(&args(&["--", "a.figcraft", "b.figcraft"]));
    assert_eq!(a.files, args(&["a.figcraft", "b.figcraft"]));
    assert!(a.ignored.is_empty());
}
