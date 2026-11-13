//! Persisted user settings (`<config dir>/wirecraft/settings.json`).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use wirecraft_core::{InterceptConfig, Rules};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub port: u16,
    /// Listen on all interfaces (phones, other machines) instead of loopback only.
    pub listen_all: bool,
    pub intercept: InterceptConfig,
    pub onboarded: bool,
    pub system_proxy_bypass: String,
    /// "system" (follow the OS), "dark" or "light".
    pub theme: String,
    pub favourites: Vec<String>,
    /// Map Local / Map Remote / Rewrite / Breakpoints / Block / Throttle.
    pub rules: Rules,
    /// WireCraft switched the OS proxy on and hasn't switched it off yet. Persisted so that after a crash or a
    /// kill the next launch still restores it on quit (or at once, if the proxy can't start).
    pub system_proxy_owned: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 9090,
            listen_all: false,
            intercept: InterceptConfig::default(),
            onboarded: false,
            system_proxy_bypass: "localhost, 127.0.0.1, ::1, *.local, 169.254/16".into(),
            theme: "system".into(),
            favourites: vec![],
            rules: Rules { map_local_on: true, map_remote_on: true, rewrite_on: true, breakpoints_on: true, scripts_on: true, ..Default::default() },
            system_proxy_owned: false,
        }
    }
}

fn path() -> PathBuf {
    wirecraft_core::paths::config_dir().join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let p = path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, s);
        }
    }
}
