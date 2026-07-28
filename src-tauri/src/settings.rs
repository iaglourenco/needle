use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "pt-BR")]
    PtBr,
    #[serde(rename = "en")]
    En,
}

impl Default for Language {
    fn default() -> Self {
        Language::PtBr
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub waiting_timeout_secs: i64,
    pub stale_timeout_secs: i64,
    pub autostart: bool,
    #[serde(default)]
    pub language: Language,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            waiting_timeout_secs: 60,
            stale_timeout_secs: 30 * 60,
            autostart: false,
            language: Language::PtBr,
        }
    }
}

fn file_path(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

pub fn load(dir: &Path) -> Settings {
    let path = file_path(dir);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

pub fn save(dir: &Path, settings: &Settings) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let pretty = serde_json::to_string_pretty(settings).unwrap_or_default();
    std::fs::write(file_path(dir), pretty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_returns_defaults_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let settings = load(dir.path());
        assert_eq!(settings.waiting_timeout_secs, 60);
        assert_eq!(settings.stale_timeout_secs, 1800);
        assert!(!settings.autostart);
        assert_eq!(settings.language, Language::PtBr);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            waiting_timeout_secs: 90,
            stale_timeout_secs: 600,
            autostart: true,
            language: Language::En,
        };
        save(dir.path(), &settings).unwrap();
        let loaded = load(dir.path());
        assert_eq!(loaded.waiting_timeout_secs, 90);
        assert_eq!(loaded.stale_timeout_secs, 600);
        assert!(loaded.autostart);
        assert_eq!(loaded.language, Language::En);
    }

    #[test]
    fn missing_language_key_in_json_defaults_to_pt_br() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"waitingTimeoutSecs":60,"staleTimeoutSecs":1800,"autostart":false}"#,
        )
        .unwrap();
        let settings = load(dir.path());
        assert_eq!(settings.language, Language::PtBr);
    }
}
