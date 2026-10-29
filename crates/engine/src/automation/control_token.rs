//! Where the desktop app keeps the session token of its control channel. The app writes it and
//! `figcraft-cli mcp --attach` reads it, both through these functions, so they always name the same
//! file.

use std::path::{Path, PathBuf};

/// The app's configuration folder (preferences, recovery copies, plugins, control tokens); `None`
/// where the system names no home or configuration folder.
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"));
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    #[cfg(not(any(unix, windows)))]
    let base: Option<PathBuf> = None;
    base.map(|b| b.join("FigCraft"))
}

/// The file holding the token of the control channel listening on `port`.
pub fn control_token_path(config_dir: &Path, port: u16) -> PathBuf {
    config_dir.join(format!("control-token-{port}"))
}
