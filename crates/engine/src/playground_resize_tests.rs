//! `playground.resize` sets the window of the page the capture service shows, and keeps what it is
//! not given.

use serde_json::json;

use crate::playground_support::Bench;

const RESIZE: &str = "playground.resize";

#[test]
fn resize_relays_the_window_and_keeps_what_is_absent() {
    let mut bench = Bench::ready("resize-keep");
    bench.run(RESIZE, &json!({"width": 900, "height": 600, "scale": 2})).unwrap();
    bench.run(RESIZE, &json!({"width": 1000})).unwrap();
    let sent = bench.calls.playground(RESIZE);
    assert_eq!(sent, [json!({"width": 900, "height": 600, "scale": 2.0}), json!({"width": 1000, "height": 600, "scale": 2.0})]);
}

#[test]
fn resize_refuses_a_window_out_of_bounds_and_a_playground_that_is_not_ready() {
    let mut bench = Bench::ready("resize-bounds");
    for params in [json!({"width": 100}), json!({"height": 5000}), json!({"scale": 3}), json!({"width": 640.5}), json!({"width": "wide"})] {
        assert!(bench.run(RESIZE, &params).is_err(), "{params} is refused");
    }
    assert!(bench.calls.playground(RESIZE).is_empty());
    bench.run("playground.close", &json!({})).unwrap();
    let refused = bench.run(RESIZE, &json!({"width": 800, "height": 600}));
    assert!(refused.is_err(), "a closed playground has no window to resize");
    assert!(bench.calls.playground(RESIZE).is_empty());
}
