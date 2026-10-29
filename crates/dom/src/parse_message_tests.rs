use super::*;

const MESSAGE_LIMIT: usize = 400;

fn snapshot_with_root(root: &str) -> String {
    format!(r#"{{"version":1,"viewport":{{"width":400,"height":200,"deviceScaleFactor":1}},"root":{root}}}"#)
}

fn refusal(json: &str) -> String {
    match parse_snapshot(json) {
        Ok(_) => panic!("expected the snapshot to be refused"),
        Err(error) => error.to_string(),
    }
}

// covers: D2
#[test]
fn a_huge_tag_is_shortened_in_the_invalid_box_message() {
    let tag = "x".repeat(100_000);
    let json = snapshot_with_root(&format!(r#"{{"type":"element","tag":"{tag}","box":{{"x":0,"y":0,"width":-1,"height":10}}}}"#));

    let message = refusal(&json);

    assert!(message.len() < MESSAGE_LIMIT, "{} bytes", message.len());
    assert!(message.contains("box") && message.contains("xxxx"), "{message}");
}

// covers: D2
#[test]
fn a_huge_node_type_is_shortened_in_the_json_message() {
    let kind = "v".repeat(100_000);
    let json = snapshot_with_root(&format!(r#"{{"type":"{kind}","box":{{"x":0,"y":0,"width":1,"height":1}}}}"#));

    let message = refusal(&json);

    assert!(message.len() < MESSAGE_LIMIT, "{} bytes", message.len());
    assert!(message.to_lowercase().contains("json") && message.contains("vvvv"), "{message}");
}

// covers: D2
#[test]
fn a_short_cause_is_kept_whole() {
    let json = snapshot_with_root(r#"{"type":"video","box":{"x":0,"y":0,"width":1,"height":1}}"#);

    assert!(refusal(&json).contains("video"));
}
