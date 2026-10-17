//! AGN Core - Shared types, configuration, and utilities.

pub mod config;
pub mod chat;
pub mod stats;
pub mod error;

pub use config::*;
pub use chat::*;
pub use stats::*;
pub use error::*;

/// Generate a new unique ID.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Get the AGN project data directory.
/// On Android: /storage/emulated/0/Download/agn
/// On other platforms: ~/Download/agn (or fallback to ~/.agn)
pub fn get_data_dir() -> std::path::PathBuf {
    #[cfg(target_os = "android")]
    {
        let p = std::path::PathBuf::from("/storage/emulated/0/Download/agn");
        if p.exists() || std::path::Path::new("/storage/emulated/0/Download").exists() {
            return p;
        }
    }

    // Try ~/Download/agn
    if let Some(home) = std::env::var_os("HOME") {
        let p = std::path::PathBuf::from(&home).join("Download").join("agn");
        if p.exists() {
            return p;
        }
        // Fallback to ~/.agn
        let fallback = std::path::PathBuf::from(&home).join(".agn");
        if fallback.exists() {
            return fallback;
        }
        // Create the Download/agn path
        let download = std::path::PathBuf::from(&home).join("Download");
        if download.exists() {
            return p;
        }
        return fallback;
    }

    std::path::PathBuf::from("./agn")
}

/// Ensure the data directory exists, creating it if necessary.
pub fn ensure_data_dir() -> std::path::PathBuf {
    let dir = get_data_dir();
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    dir
}