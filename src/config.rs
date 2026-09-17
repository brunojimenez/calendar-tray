//! Carga y guardado de la configuración local del usuario (SPEC.md §3, §2.8).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CalendarSourceKind {
    Ics,
    GoogleApi,
}

impl Default for CalendarSourceKind {
    fn default() -> Self {
        CalendarSourceKind::Ics
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    Light,
    Dark,
    System,
}

impl Default for ThemePreference {
    fn default() -> Self {
        ThemePreference::System
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// URL secreta del feed ICS. Vacía = app "sin configurar" (SPEC.md §2.6).
    pub ics_feed_url: String,
    pub source: CalendarSourceKind,
    pub green_threshold_minutes: u32,
    pub yellow_threshold_minutes: u32,
    pub blink_threshold_minutes: u32,
    pub snooze_minutes: u32,
    pub do_not_disturb_max_minutes: u32,
    pub theme: ThemePreference,
    pub autostart: bool,
    pub refresh_interval_minutes: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            ics_feed_url: String::new(),
            source: CalendarSourceKind::default(),
            green_threshold_minutes: 15,
            yellow_threshold_minutes: 5,
            blink_threshold_minutes: 1,
            snooze_minutes: 2,
            do_not_disturb_max_minutes: 30,
            theme: ThemePreference::default(),
            autostart: false,
            refresh_interval_minutes: 5,
        }
    }
}

impl AppConfig {
    /// `%APPDATA%\CalendarTray\config.toml` (SPEC.md §3: misma carpeta que la versión Java).
    pub fn config_path() -> PathBuf {
        let appdata = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        appdata.join("CalendarTray").join("config.toml")
    }

    pub fn load_or_default() -> Self {
        match std::fs::read_to_string(Self::config_path()) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
            Err(_) => AppConfig::default(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self).expect("AppConfig siempre serializa a TOML");
        std::fs::write(path, contents)
    }

    /// Primer arranque / sin URL guardada todavía (SPEC.md §2.6).
    pub fn is_unconfigured(&self) -> bool {
        self.ics_feed_url.trim().is_empty()
    }
}
