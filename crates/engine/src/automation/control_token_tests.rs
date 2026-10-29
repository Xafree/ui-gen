//! Where the control channel's session token lives, for the app and for its clients alike.

use std::path::Path;

use super::control_token::{config_dir, control_token_path};

#[test]
fn the_token_file_is_named_after_the_port_in_the_configuration_folder() {
    let dir = Path::new("config");
    assert_eq!(control_token_path(dir, 7979), dir.join("control-token-7979"));
    assert_ne!(control_token_path(dir, 7979), control_token_path(dir, 7980), "one file per port");
}

#[test]
fn the_configuration_folder_is_the_apps_own() {
    if let Some(dir) = config_dir() {
        assert_eq!(dir.file_name().and_then(|n| n.to_str()), Some("FigCraft"));
    }
}
