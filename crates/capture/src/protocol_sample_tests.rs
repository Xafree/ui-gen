//! The `render` request carries the sample text in its own field, and only when there is one.

use figcraft_engine::capture::CaptureJob;
use serde_json::json;

use crate::config::Timeouts;
use crate::protocol::request_of;

fn render(sample_text: Option<&str>) -> CaptureJob {
    CaptureJob::Render {
        root: "root".to_string(),
        package: "@acme/ui".to_string(),
        component: "Button".to_string(),
        props: json!({"variant": "primary"}),
        sample_text: sample_text.map(str::to_string),
        state: "default".to_string(),
        theme: "light".to_string(),
    }
}

#[test]
fn the_sample_text_is_a_field_of_the_request_next_to_the_props() {
    let request = request_of(render(Some("Button")), &Timeouts::default()).unwrap();

    assert_eq!(request.params["sampleText"], json!("Button"));
    assert_eq!(request.params["props"], json!({"variant": "primary"}));
}

#[test]
fn without_a_sample_text_the_request_has_no_such_field() {
    let request = request_of(render(None), &Timeouts::default()).unwrap();

    assert!(request.params.get("sampleText").is_none(), "{}", request.params);
}
