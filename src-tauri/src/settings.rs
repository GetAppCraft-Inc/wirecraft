//! Persisted user settings (`<config dir>/xwire/settings.json`).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use xwire_core::InterceptConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub port: u16,
    /// Listen on all interfaces (phones, other machines) instead of loopback only.
    pub listen_all: bool,
    pub intercept: InterceptConfig,
    pub onboarded: bool,
    pub system_proxy_bypass: String,
    pub theme: String,
    pub favourites: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 9090,
            listen_all: false,
            intercept: InterceptConfig::default(),
            onboarded: false,
            system_proxy_bypass: "localhost, 127.0.0.1, ::1, *.local, 169.254/16".into(),
            theme: "dark".into(),
            favourites: vec![],
        }
    }
}

fn path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("xwire").join("settings.json")
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
