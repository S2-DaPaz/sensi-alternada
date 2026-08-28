//! Panel settings, persisted as JSON next to the user's profile.

use crate::fire_button::FireButton;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub base_dpi: u32,
    /// Zero means "follow `base_dpi`" — that is what a file written before this field
    /// existed deserialises to.
    #[serde(default)]
    pub base_dpi_y: u32,
    pub shooting_dpi_x: u32,
    pub shooting_dpi_y: u32,
    pub split_axes: bool,
    pub fire_button: FireButton,
    pub enabled: bool,
    /// A file written before the colours existed must not deserialise them to black.
    #[serde(default = "default_accent")]
    pub accent: [u8; 3],
    #[serde(default = "default_background")]
    pub background: [u8; 3],
}

fn default_accent() -> [u8; 3] {
    [96, 205, 255]
}

fn default_background() -> [u8; 3] {
    [22, 24, 28]
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            base_dpi: 800,
            base_dpi_y: 800,
            shooting_dpi_x: 400,
            shooting_dpi_y: 400,
            split_axes: false,
            fire_button: FireButton::Left,
            enabled: false,
            accent: default_accent(),
            background: default_background(),
        }
    }
}

impl Settings {
    /// Never fails: a missing, truncated or hand-edited file must not stop the panel
    /// from opening.
    pub fn from_json_str(raw: &str) -> Self {
        let mut settings: Self = serde_json::from_str(raw).unwrap_or_default();
        if settings.base_dpi_y == 0 {
            settings.base_dpi_y = settings.base_dpi;
        }
        settings
    }

    fn path() -> Option<std::path::PathBuf> {
        Some(
            dirs::config_dir()?
                .join("sensi-alternada")
                .join("settings.json"),
        )
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|raw| Self::from_json_str(&raw))
            .unwrap_or_default()
    }

    /// Best effort: failing to persist must never interrupt play.
    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, raw);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Adding a field must not make an existing file unreadable: `unwrap_or_default()`
    // would silently throw the user's whole configuration away.
    #[test]
    fn a_file_written_before_the_y_base_existed_keeps_its_values() {
        let old = r#"{
            "base_dpi": 1600,
            "shooting_dpi_x": 500,
            "shooting_dpi_y": 250,
            "split_axes": true,
            "fire_button": "Right",
            "enabled": true
        }"#;
        let loaded = Settings::from_json_str(old);
        assert_eq!(loaded.base_dpi, 1600);
        assert_eq!(
            loaded.base_dpi_y, 1600,
            "o eixo Y novo herda a base existente"
        );
    }

    #[test]
    fn a_file_without_colours_gets_the_real_defaults_not_black() {
        let old = r#"{
            "base_dpi": 800,
            "base_dpi_y": 800,
            "shooting_dpi_x": 400,
            "shooting_dpi_y": 400,
            "split_axes": false,
            "fire_button": "Left",
            "enabled": false
        }"#;
        let loaded = Settings::from_json_str(old);
        assert_eq!(loaded.accent, default_accent());
        assert_eq!(loaded.background, default_background(), "preto no preto");
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let saved = Settings {
            base_dpi: 1600,
            base_dpi_y: 1200,
            shooting_dpi_x: 500,
            shooting_dpi_y: 250,
            split_axes: true,
            fire_button: FireButton::Side2,
            enabled: true,
            accent: [200, 40, 90],
            background: [250, 248, 245],
        };
        let raw = serde_json::to_string(&saved).unwrap();
        assert_eq!(Settings::from_json_str(&raw), saved);
    }

    #[test]
    fn corrupt_json_falls_back_to_defaults() {
        assert_eq!(Settings::from_json_str("{ not json"), Settings::default());
    }
}
