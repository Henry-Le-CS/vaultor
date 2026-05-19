use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ── GitRemoteConfig ──────────────────────────────────────────────────────────

fn generate_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Persisted connection parameters for one git remote repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRemoteConfig {
    /// Stable local identifier used as the clone directory name (`git-repos/{id}/`).
    /// Generated on first connect; never changes for a given remote.
    #[serde(default = "generate_id")]
    pub id: String,
    pub url: String,
    pub branch: String,
    /// Unix-millisecond timestamp of the last successful sync. `None` if never synced.
    pub last_synced: Option<i64>,
}

// ── AppSettings ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    /// Custom vault DB path. `None` means use the default (`app_data_dir/vaultor.db`).
    #[serde(default)]
    pub db_path: Option<PathBuf>,

    /// How long an unlocked session stays active.
    #[serde(default)]
    pub session_expiry: SessionExpiry,

    /// All connected git repositories.
    #[serde(default)]
    pub git_remotes: Vec<GitRemoteConfig>,

    /// URL of the currently-active repository. `None` means local SQLite mode.
    #[serde(default)]
    pub active_git_url: Option<String>,

    /// Legacy v1 single-remote field — read from old settings.json for migration,
    /// never written on save.
    #[serde(default, skip_serializing)]
    pub git_remote: Option<GitRemoteConfig>,

    /// Whether the user has seen the onboarding tutorial.
    #[serde(default)]
    pub tutorial_seen: bool,
}

impl AppSettings {
    /// Load from `<config_dir>/settings.json`, falling back to defaults on any
    /// error (missing file, corrupt JSON, etc.).
    ///
    /// Migrates the old `git_remote` single-remote format to `git_remotes` on
    /// first load if the new list is empty.
    pub fn load_or_default(config_dir: &Path) -> Self {
        let path = config_dir.join("settings.json");
        let mut s: Self = match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        };
        // Migrate legacy single-remote → multi-repo list.
        if s.git_remotes.is_empty() {
            if let Some(old) = s.git_remote.take() {
                let url = old.url.clone();
                s.git_remotes.push(old);
                if s.active_git_url.is_none() {
                    s.active_git_url = Some(url);
                }
            }
        }
        s
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

    // ── Multi-repo helpers ────────────────────────────────────────────────────

    /// Return the currently-active remote, if any.
    pub fn active_git_remote(&self) -> Option<&GitRemoteConfig> {
        let url = self.active_git_url.as_deref()?;
        self.git_remotes.iter().find(|r| r.url == url)
    }

    /// Return a mutable reference to the currently-active remote, if any.
    pub fn active_git_remote_mut(&mut self) -> Option<&mut GitRemoteConfig> {
        let url = self.active_git_url.as_deref()?.to_owned();
        self.git_remotes.iter_mut().find(|r| r.url == url)
    }

    /// Add or replace a remote and make it the active one.
    pub fn add_git_remote(&mut self, config: GitRemoteConfig) {
        let url = config.url.clone();
        if let Some(existing) = self.git_remotes.iter_mut().find(|r| r.url == config.url) {
            *existing = config;
        } else {
            self.git_remotes.push(config);
        }
        self.active_git_url = Some(url);
    }

    /// Remove a remote by URL. If it was active, the first remaining remote
    /// (if any) becomes active; otherwise `active_git_url` is cleared.
    pub fn remove_git_remote(&mut self, url: &str) {
        self.git_remotes.retain(|r| r.url != url);
        if self.active_git_url.as_deref() == Some(url) {
            self.active_git_url = self.git_remotes.first().map(|r| r.url.clone());
        }
    }

    /// Switch the active remote to `url`.  Returns `false` if no remote with
    /// that URL exists.
    pub fn set_active_git_remote(&mut self, url: &str) -> bool {
        if self.git_remotes.iter().any(|r| r.url == url) {
            self.active_git_url = Some(url.to_owned());
            true
        } else {
            false
        }
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

    fn make_remote(url: &str) -> GitRemoteConfig {
        GitRemoteConfig {
            id: "test-id".to_string(),
            url: url.to_string(),
            branch: "main".to_string(),
            last_synced: None,
        }
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
            ..Default::default()
        };

        s.save(dir.path()).unwrap();
        let loaded = AppSettings::load_or_default(dir.path());

        assert_eq!(loaded.session_expiry, SessionExpiry::Minutes10);
        assert_eq!(loaded.db_path, Some(PathBuf::from("/tmp/test.db")));
        assert!(loaded.git_remotes.is_empty());
        assert!(loaded.active_git_url.is_none());
    }

    #[test]
    fn roundtrip_with_git_remote() {
        let dir = tmp();
        let mut s = AppSettings {
            session_expiry: SessionExpiry::Minutes2,
            db_path: None,
            ..Default::default()
        };
        s.add_git_remote(GitRemoteConfig {
            id: "abc123".to_string(),
            url: "git@github.com:user/vault.git".to_string(),
            branch: "main".to_string(),
            last_synced: Some(1_715_000_000_000),
        });

        s.save(dir.path()).unwrap();
        let loaded = AppSettings::load_or_default(dir.path());

        let remote = loaded
            .active_git_remote()
            .expect("active_git_remote must be set");
        assert_eq!(remote.url, "git@github.com:user/vault.git");
        assert_eq!(remote.branch, "main");
        assert_eq!(remote.last_synced, Some(1_715_000_000_000));
        assert_eq!(
            loaded.active_git_url.as_deref(),
            Some("git@github.com:user/vault.git")
        );
    }

    #[test]
    fn migrates_legacy_git_remote_field() {
        let dir = tmp();
        // Old-format settings.json with single git_remote field.
        fs::write(
            dir.path().join("settings.json"),
            r#"{"git_remote":{"url":"git@github.com:user/vault.git","branch":"main","last_synced":null}}"#,
        )
        .unwrap();
        let loaded = AppSettings::load_or_default(dir.path());
        assert_eq!(loaded.git_remotes.len(), 1);
        assert_eq!(
            loaded.active_git_url.as_deref(),
            Some("git@github.com:user/vault.git")
        );
        let remote = loaded.active_git_remote().unwrap();
        assert_eq!(remote.url, "git@github.com:user/vault.git");
    }

    #[test]
    fn missing_git_remote_field_defaults_to_empty() {
        let dir = tmp();
        fs::write(
            dir.path().join("settings.json"),
            r#"{"session_expiry":"minutes_10"}"#,
        )
        .unwrap();
        let loaded = AppSettings::load_or_default(dir.path());
        assert_eq!(loaded.session_expiry, SessionExpiry::Minutes10);
        assert!(loaded.git_remotes.is_empty());
        assert!(loaded.active_git_url.is_none());
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

    #[test]
    fn add_git_remote_replaces_existing() {
        let mut s = AppSettings::default();
        s.add_git_remote(make_remote("git@github.com:user/repo.git"));
        s.add_git_remote(GitRemoteConfig {
            id: "new-id".to_string(),
            url: "git@github.com:user/repo.git".to_string(),
            branch: "develop".to_string(),
            last_synced: None,
        });
        assert_eq!(s.git_remotes.len(), 1);
        assert_eq!(s.git_remotes[0].branch, "develop");
    }

    #[test]
    fn remove_git_remote_switches_active() {
        let mut s = AppSettings::default();
        s.add_git_remote(make_remote("git@github.com:user/repo1.git"));
        s.add_git_remote(make_remote("git@github.com:user/repo2.git"));
        // repo2 is now active (last added)
        s.remove_git_remote("git@github.com:user/repo2.git");
        assert_eq!(s.git_remotes.len(), 1);
        assert_eq!(
            s.active_git_url.as_deref(),
            Some("git@github.com:user/repo1.git")
        );
    }

    #[test]
    fn set_active_git_remote_returns_false_for_unknown() {
        let mut s = AppSettings::default();
        s.add_git_remote(make_remote("git@github.com:user/repo.git"));
        assert!(!s.set_active_git_remote("git@github.com:user/other.git"));
        assert_eq!(
            s.active_git_url.as_deref(),
            Some("git@github.com:user/repo.git")
        );
    }
}
