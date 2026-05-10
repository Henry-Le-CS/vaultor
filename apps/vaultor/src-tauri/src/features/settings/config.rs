use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ── AppSettings ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    /// Custom vault DB path. `None` means use the default (`app_data_dir/vaultor.db`).
    #[serde(default)]
    pub db_path: Option<PathBuf>,

    /// How long an unlocked session stays active.
    #[serde(default)]
    pub session_expiry: SessionExpiry,
}

impl AppSettings {
    /// Load from `<config_dir>/settings.json`, falling back to defaults on any
    /// error (missing file, corrupt JSON, etc.).
    pub fn load_or_default(config_dir: &Path) -> Self {
        let path = config_dir.join("settings.json");
        match fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist to `<config_dir>/settings.json`.
    pub fn save(&self, config_dir: &Path) -> Result<(), String> {
        let path = config_dir.join("settings.json");
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, json).map_err(|e| e.to_string())
    }

    /// Return the vault DB path, resolving to the default when unset.
    pub fn resolved_db_path(&self, data_dir: &Path) -> PathBuf {
        self.db_path
            .clone()
            .unwrap_or_else(|| data_dir.join("vaultor.db"))
    }
}

// ── SessionExpiry ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum SessionExpiry {
    /// 2-minute session (default).
    #[default]
    #[serde(rename = "minutes_2")]
    Minutes2,
    /// 5-minute session.
    #[serde(rename = "minutes_5")]
    Minutes5,
    /// 10-minute session.
    #[serde(rename = "minutes_10")]
    Minutes10,
    /// Session lives until the app quits or the user manually locks.
    #[serde(rename = "until_quit")]
    UntilQuit,
}

impl SessionExpiry {
    /// Returns the TTL duration, or `None` for `UntilQuit`.
    pub fn to_duration(&self) -> Option<Duration> {
        match self {
            Self::Minutes2 => Some(Duration::from_secs(120)),
            Self::Minutes5 => Some(Duration::from_secs(300)),
            Self::Minutes10 => Some(Duration::from_secs(600)),
            Self::UntilQuit => None,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn tmp() -> TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn load_or_default_when_absent() {
        let dir = tmp();
        let s = AppSettings::load_or_default(dir.path());
        assert_eq!(s.session_expiry, SessionExpiry::Minutes2);
        assert!(s.db_path.is_none());
    }

    #[test]
    fn roundtrip_save_and_load() {
        let dir = tmp();
        let s = AppSettings {
            session_expiry: SessionExpiry::Minutes10,
            db_path: Some(PathBuf::from("/tmp/test.db")),
        };

        s.save(dir.path()).unwrap();
        let loaded = AppSettings::load_or_default(dir.path());

        assert_eq!(loaded.session_expiry, SessionExpiry::Minutes10);
        assert_eq!(loaded.db_path, Some(PathBuf::from("/tmp/test.db")));
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = tmp();
        fs::write(dir.path().join("settings.json"), b"not json").unwrap();
        let s = AppSettings::load_or_default(dir.path());
        assert_eq!(s.session_expiry, SessionExpiry::Minutes2);
    }

    #[test]
    fn resolved_db_path_uses_default_when_unset() {
        let data = PathBuf::from("/data");
        let s = AppSettings::default();
        assert_eq!(s.resolved_db_path(&data), PathBuf::from("/data/vaultor.db"));
    }

    #[test]
    fn resolved_db_path_uses_custom_when_set() {
        let data = PathBuf::from("/data");
        let s = AppSettings {
            db_path: Some(PathBuf::from("/custom/vault.db")),
            ..Default::default()
        };
        assert_eq!(s.resolved_db_path(&data), PathBuf::from("/custom/vault.db"));
    }

    #[test]
    fn session_expiry_to_duration() {
        assert_eq!(
            SessionExpiry::Minutes2.to_duration(),
            Some(Duration::from_secs(120))
        );
        assert_eq!(
            SessionExpiry::Minutes5.to_duration(),
            Some(Duration::from_secs(300))
        );
        assert_eq!(
            SessionExpiry::Minutes10.to_duration(),
            Some(Duration::from_secs(600))
        );
        assert_eq!(SessionExpiry::UntilQuit.to_duration(), None);
    }
}
