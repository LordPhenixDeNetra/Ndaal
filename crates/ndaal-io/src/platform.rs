//! Defaults that depend on the operating system (CLAUDE.md section 4.8:
//! system-specific code lives here, always with a fallback).

use std::path::PathBuf;

/// Relative fallback when the per-user directory is unknown.
const FALLBACK_DATA_DIR: &str = "ndaal-data";

/// Default data directory (`node.data_dir`, P-34). `var` reads an environment
/// variable; it is a parameter so that tests choose the environment.
///
/// - Linux and other systems: `/var/lib/ndaal` (packages and containers);
/// - macOS: `$HOME/Library/Application Support/ndaal`;
/// - Windows: `%LOCALAPPDATA%\ndaal\data`.
///
/// On macOS and Windows, without the variable: `ndaal-data`, relative to the
/// current directory.
pub fn default_data_dir(var: impl Fn(&str) -> Option<String>) -> PathBuf {
    let per_user = if cfg!(target_os = "windows") {
        var("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("ndaal").join("data"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("ndaal")
        })
    } else {
        return PathBuf::from("/var/lib/ndaal");
    };
    per_user
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from(FALLBACK_DATA_DIR))
}
