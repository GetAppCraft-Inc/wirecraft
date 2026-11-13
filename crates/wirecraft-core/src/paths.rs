//! Where WireCraft keeps its files, and the one-time move from the pre-rename `xwire` folders.

use std::path::{Path, PathBuf};

/// Folder name under the platform data / config directories.
pub const APP_DIR: &str = "wirecraft";
/// The project was called xwire before 0.2.0.
const LEGACY_DIR: &str = "xwire";

/// `<base>/wirecraft`, moving a pre-rename `<base>/xwire` into place the first time so the root CA (and with
/// it every device that already trusts it), settings and sessions carry over.
pub fn app_dir(base: Option<PathBuf>) -> PathBuf {
    let base = base.unwrap_or_else(std::env::temp_dir);
    let dir = base.join(APP_DIR);
    migrate(&base.join(LEGACY_DIR), &dir);
    dir
}

fn migrate(legacy: &Path, dir: &Path) {
    if !dir.exists() && legacy.is_dir() {
        match std::fs::rename(legacy, dir) {
            Ok(()) => tracing::info!("moved {} to {}", legacy.display(), dir.display()),
            Err(e) => tracing::warn!("could not move {} to {}: {e}", legacy.display(), dir.display()),
        }
    }
}

/// Application data: the root CA and saved sessions.
pub fn data_dir() -> PathBuf {
    app_dir(dirs::data_dir())
}

/// Settings.
pub fn config_dir() -> PathBuf {
    app_dir(dirs::config_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_legacy_folder_once() {
        let base = std::env::temp_dir().join(format!("wirecraft-paths-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("xwire/ca")).unwrap();
        std::fs::write(base.join("xwire/ca/x"), "1").unwrap();
        let d = app_dir(Some(base.clone()));
        assert_eq!(d, base.join("wirecraft"));
        assert!(d.join("ca/x").exists());
        assert!(!base.join("xwire").exists());
        // a later legacy folder is left alone once the new one exists
        std::fs::create_dir_all(base.join("xwire")).unwrap();
        app_dir(Some(base.clone()));
        assert!(base.join("xwire").exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
