//! Git subprocess wrapper.
//!
//! Wraps every `git` call as a `std::process::Command`. Auth is handled
//! entirely by the system — SSH agent, osxkeychain helper, ~/.gitconfig —
//! with no credentials stored by the app.
//!
//! All calls are logged via `tracing`. HTTPS URLs with embedded credentials
//! are sanitized before any log write.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::VaultError;

// ── SSH helper ───────────────────────────────────────────────────────────────

/// Return a `git` `Command` pre-configured with a rich PATH and `GIT_SSH_COMMAND`.
///
/// macOS GUI apps (launched via Tauri) inherit a minimal environment PATH
/// (`/usr/bin:/bin:/usr/sbin:/sbin`) that does not include Homebrew or
/// Xcode CLT locations.  We augment it so `git` is found regardless of how
/// it was installed:
///
/// - `/usr/bin`          — Xcode Command Line Tools (the most common location)
/// - `/usr/local/bin`    — Homebrew on Intel Macs
/// - `/opt/homebrew/bin` — Homebrew on Apple Silicon
///
/// `GIT_SSH_COMMAND` silently accepts new host keys on first connect
/// (prevents the interactive "Are you sure?" prompt for github.com etc.).
fn git_cmd() -> Command {
    let mut cmd = Command::new("git");
    cmd.env(
        "PATH",
        "/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:/opt/homebrew/bin:/opt/homebrew/sbin",
    );
    cmd.env("GIT_SSH_COMMAND", "ssh -o StrictHostKeyChecking=accept-new");
    cmd
}

// ── URL sanitization ─────────────────────────────────────────────────────────

/// Strip embedded credentials from an HTTP/HTTPS URL before logging.
///
/// `https://user:token@github.com/repo` → `https://***@github.com/repo`
///
/// Only HTTP/HTTPS URLs are affected. SSH URLs (`git@host`, `ssh://user@host`)
/// use the `user@host` pattern for the host address, not for credentials, so
/// they are returned unchanged.
pub fn sanitize_url(url: &str) -> String {
    // Only sanitize http/https — SSH URLs don't embed credentials this way.
    let is_http = url.starts_with("http://") || url.starts_with("https://");
    if !is_http {
        return url.to_string();
    }

    if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        if let Some(at_pos) = after_scheme.find('@') {
            let scheme = &url[..scheme_end + 3];
            let host_and_rest = &after_scheme[at_pos..]; // includes the '@'
            return format!("{scheme}***{host_and_rest}");
        }
    }
    url.to_string()
}

// ── Input validation ─────────────────────────────────────────────────────────

/// Validate that a remote URL uses a known safe scheme.
/// Prevents shell injection via malicious URLs passed to subprocess args.
pub fn validate_url(url: &str) -> Result<(), VaultError> {
    let ok = url.starts_with("https://")
        || url.starts_with("http://")
        || url.starts_with("git@")
        || url.starts_with("ssh://");
    if ok {
        Ok(())
    } else {
        Err(VaultError::Validation(format!(
            "unsupported remote URL scheme: {url}"
        )))
    }
}

/// Validate that a branch name contains only safe characters.
/// Prevents shell injection via branch names passed to subprocess args.
pub fn validate_branch(branch: &str) -> Result<(), VaultError> {
    let ok = !branch.is_empty()
        && branch.len() <= 200
        && branch
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '.'));
    if ok {
        Ok(())
    } else {
        Err(VaultError::Validation(format!(
            "invalid branch name: {branch}"
        )))
    }
}

// ── GitRunner ────────────────────────────────────────────────────────────────

/// Runs git subprocess commands against a local working directory.
///
/// For operations that do not require a working directory (e.g. `ls-remote`),
/// `repo_path` is not used — pass any path or use the dedicated free function.
pub struct GitRunner {
    repo_path: PathBuf,
}

impl GitRunner {
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
        }
    }

    /// Run a git command inside `repo_path` and return trimmed stdout.
    #[allow(dead_code)]
    fn run(&self, args: &[&str]) -> Result<String, VaultError> {
        run_git_in(args, &self.repo_path)
    }

    // ── Remote operations ────────────────────────────────────────────────────

    /// Clone `url` at `branch` into `repo_path`.
    /// `repo_path` must NOT already exist.
    pub fn clone_repo(&self, url: &str, branch: &str) -> Result<(), VaultError> {
        // Ensure the parent directory exists — git cannot clone into a path whose
        // parent doesn't exist, and `Command::current_dir` fails with ENOENT if
        // the directory is missing.
        let parent = self.repo_path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)
            .map_err(|e| VaultError::Io(format!("failed to create clone parent dir: {e}")))?;

        let dir_name = self
            .repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("git-repo");

        let output = git_cmd()
            .args([
                "clone",
                "--branch",
                branch,
                "--no-local",
                "--depth",
                "1",
                url,
                dir_name,
            ])
            .current_dir(parent)
            .output()
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;

        log_result("git clone", &sanitize_url(url), &output);

        if output.status.success() {
            Ok(())
        } else {
            Err(git_error("git clone", &output))
        }
    }

    /// Fetch the latest state of `branch` from origin.
    pub fn fetch(&self, branch: &str) -> Result<(), VaultError> {
        let refspec = format!("refs/heads/{branch}:refs/remotes/origin/{branch}");
        let output = self
            .run_raw(&["fetch", "origin", &refspec])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;

        log_result("git fetch", branch, &output);

        if output.status.success() {
            Ok(())
        } else {
            Err(git_error("git fetch", &output))
        }
    }

    /// List file names in a remote directory tree (one level deep).
    ///
    /// Example: `list_remote_dir("main", "vault-data/secrets/")` returns
    /// `["abc.json", "def.json"]`.
    pub fn list_remote_dir(&self, branch: &str, path: &str) -> Result<Vec<String>, VaultError> {
        let tree_ref = format!("origin/{branch}:{path}");
        let output = self
            .run_raw(&["ls-tree", "--name-only", &tree_ref])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // If the path doesn't exist yet (empty repo initial push), return empty.
            if stderr.contains("Not a valid object name")
                || stderr.contains("fatal: not a tree object")
            {
                return Ok(vec![]);
            }
            log_result("git ls-tree", path, &output);
            return Err(git_error("git ls-tree", &output));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let names = stdout
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        Ok(names)
    }

    /// Read the content of a file from the remote ref (without checkout).
    ///
    /// Example: `show_remote_file("main", "vault-data/secrets/abc.json")`
    pub fn show_remote_file(&self, branch: &str, path: &str) -> Result<String, VaultError> {
        let object = format!("origin/{branch}:{path}");
        let output = self
            .run_raw(&["show", &object])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;

        if !output.status.success() {
            log_result("git show", path, &output);
            return Err(git_error("git show", &output));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| VaultError::Io(format!("git show output is not valid UTF-8: {e}")))
    }

    /// Stage all changes in `vault-data/`.
    pub fn add_vault_data(&self) -> Result<(), VaultError> {
        let output = self
            .run_raw(&["add", "vault-data/"])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;

        log_result("git add", "vault-data/", &output);

        if output.status.success() {
            Ok(())
        } else {
            Err(git_error("git add", &output))
        }
    }

    /// Stage all changes in `vault-data/` — including deletions (`-A` flag).
    ///
    /// Use this during sync so that files removed from disk (stale entity files)
    /// are also recorded in the git index before committing.
    pub fn add_vault_data_all(&self) -> Result<(), VaultError> {
        let output = self
            .run_raw(&["add", "-A", "vault-data/"])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;

        log_result("git add -A", "vault-data/", &output);

        if output.status.success() {
            Ok(())
        } else {
            Err(git_error("git add -A", &output))
        }
    }

    /// Remove a file from the index and working tree.
    pub fn rm_file(&self, path: &str) -> Result<(), VaultError> {
        let output = self
            .run_raw(&["rm", "-f", path])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;

        log_result("git rm", path, &output);

        if output.status.success() {
            Ok(())
        } else {
            Err(git_error("git rm", &output))
        }
    }

    /// Create a commit. Returns `Ok(false)` if there was nothing to commit.
    pub fn commit(&self, message: &str) -> Result<bool, VaultError> {
        // Check if there is anything staged.
        let diff_output = self
            .run_raw(&["diff", "--cached", "--quiet"])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;

        if diff_output.status.success() {
            // Exit 0 means no staged changes.
            tracing::debug!(target: "vaultor::git", "nothing to commit — skipping");
            return Ok(false);
        }

        let output = self
            .run_raw(&[
                "commit",
                "--author=Vaultor Sync <noreply@vaultor>",
                "-m",
                message,
            ])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;

        log_result("git commit", message, &output);

        if output.status.success() {
            Ok(true)
        } else {
            Err(git_error("git commit", &output))
        }
    }

    /// Push to `origin/<branch>` using `--force-with-lease`.
    /// Returns `true` on success, `false` if rejected (caller should retry).
    pub fn push_force_with_lease(&self, branch: &str) -> Result<bool, VaultError> {
        let refspec = format!("HEAD:refs/heads/{branch}");
        let output = self
            .run_raw(&["push", "--force-with-lease", "origin", &refspec])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            tracing::debug!(target: "vaultor::git", command = "git push", branch, "push succeeded");
            return Ok(true);
        }

        // Detect non-fast-forward rejection (stale lease) — caller can retry.
        if stderr.contains("stale info") || stderr.contains("rejected") {
            tracing::warn!(
                target: "vaultor::git",
                command = "git push",
                branch,
                stderr = %stderr,
                "push rejected (stale lease) — caller should retry"
            );
            return Ok(false);
        }

        // Any other failure is an error.
        tracing::error!(
            target: "vaultor::git",
            command = "git push",
            branch,
            exit_code = output.status.code(),
            stderr = %stderr,
            "git push failed"
        );
        Err(VaultError::Io(stderr))
    }

    /// Returns `true` if there are staged changes under `vault-data/`.
    ///
    /// `git diff --cached --quiet` exits 0 for no changes, 1 for changes.
    /// Scoped to `vault-data/` so unrelated index state does not interfere.
    pub fn has_staged_changes(&self) -> Result<bool, VaultError> {
        let output = self
            .run_raw(&["diff", "--cached", "--quiet", "--", "vault-data/"])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;
        Ok(!output.status.success())
    }

    /// Returns the full commit hash at `HEAD`.
    pub fn local_hash(&self) -> Result<String, VaultError> {
        let output = self
            .run_raw(&["rev-parse", "HEAD"])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;
        if !output.status.success() {
            return Err(git_error("git rev-parse HEAD", &output));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Returns the full commit hash at `FETCH_HEAD`, or `None` if it does
    /// not exist yet (repo with no prior fetch).
    pub fn remote_hash(&self) -> Result<Option<String>, VaultError> {
        let output = self
            .run_raw(&["rev-parse", "FETCH_HEAD"])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;
        if !output.status.success() {
            return Ok(None);
        }
        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if hash.is_empty() {
            return Ok(None);
        }
        Ok(Some(hash))
    }

    /// Hard-reset `HEAD` to `FETCH_HEAD`, discarding any local working tree
    /// changes in favour of the just-fetched remote state.
    pub fn reset_hard_fetch_head(&self) -> Result<(), VaultError> {
        let output = self
            .run_raw(&["reset", "--hard", "FETCH_HEAD"])
            .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;
        log_result("git reset --hard", "FETCH_HEAD", &output);
        if output.status.success() {
            Ok(())
        } else {
            Err(git_error("git reset --hard FETCH_HEAD", &output))
        }
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    fn run_raw(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        git_cmd().args(args).current_dir(&self.repo_path).output()
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Run `git ls-remote --heads <url>` and return parsed branch names.
/// Does NOT require a local working directory.
pub fn ls_remote_heads(url: &str) -> Result<Vec<String>, VaultError> {
    let output = git_cmd()
        .args(["ls-remote", "--heads", url])
        .output()
        .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;

    let sanitized = sanitize_url(url);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        tracing::error!(
            target: "vaultor::git",
            command = "git ls-remote",
            url = %sanitized,
            exit_code = output.status.code(),
            stderr = %stderr,
            "git ls-remote failed"
        );
        return Err(VaultError::Io(stderr));
    }

    tracing::debug!(
        target: "vaultor::git",
        command = "git ls-remote",
        url = %sanitized,
        "ls-remote succeeded"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let branches = parse_ls_remote_heads(&stdout);
    Ok(branches)
}

/// Parse `refs/heads/<name>` lines from `git ls-remote --heads` output.
pub fn parse_ls_remote_heads(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() == 2 {
                parts[1].strip_prefix("refs/heads/").map(str::to_string)
            } else {
                None
            }
        })
        .collect()
}

/// Choose the default branch from a list: `main` > `master` > first alphabetically.
pub fn default_branch(branches: &[String]) -> Option<String> {
    for preferred in &["main", "master"] {
        if branches.iter().any(|b| b == preferred) {
            return Some(preferred.to_string());
        }
    }
    branches.first().cloned()
}

// ── Private helpers ──────────────────────────────────────────────────────────

#[allow(dead_code)]
fn run_git_in(args: &[&str], dir: &Path) -> Result<String, VaultError> {
    let output = git_cmd()
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| VaultError::Io(format!("failed to spawn git: {e}")))?;

    log_result(&format!("git {}", args.first().unwrap_or(&"")), "", &output);

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(git_error(
            &format!("git {}", args.first().unwrap_or(&"")),
            &output,
        ))
    }
}

fn log_result(command: &str, context: &str, output: &std::process::Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        tracing::debug!(
            target: "vaultor::git",
            command,
            context,
            exit_code = output.status.code(),
            "{command} succeeded"
        );
    } else {
        tracing::error!(
            target: "vaultor::git",
            command,
            context,
            exit_code = output.status.code(),
            stderr = %stderr,
            "{command} failed"
        );
    }
}

fn git_error(command: &str, output: &std::process::Output) -> VaultError {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    VaultError::Io(format!("{command} failed: {stderr}"))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_https_with_credentials() {
        assert_eq!(
            sanitize_url("https://user:token@github.com/owner/repo.git"),
            "https://***@github.com/owner/repo.git"
        );
    }

    #[test]
    fn sanitize_https_no_credentials() {
        let url = "https://github.com/owner/repo.git";
        assert_eq!(sanitize_url(url), url);
    }

    #[test]
    fn sanitize_ssh_url_unchanged() {
        let url = "git@github.com:owner/repo.git";
        assert_eq!(sanitize_url(url), url);
    }

    #[test]
    fn sanitize_ssh_scheme_unchanged() {
        let url = "ssh://git@github.com/owner/repo.git";
        // ssh:// URLs don't embed credentials in the URL itself
        assert_eq!(sanitize_url(url), url);
    }

    #[test]
    fn parse_ls_remote_heads_standard() {
        let output = "\
abc123\trefs/heads/main\n\
def456\trefs/heads/develop\n\
789abc\trefs/heads/feature/xyz\n";
        let branches = parse_ls_remote_heads(output);
        assert_eq!(branches, vec!["main", "develop", "feature/xyz"]);
    }

    #[test]
    fn parse_ls_remote_heads_empty() {
        assert!(parse_ls_remote_heads("").is_empty());
    }

    #[test]
    fn parse_ls_remote_heads_ignores_tags() {
        let output = "\
abc123\trefs/heads/main\n\
def456\trefs/tags/v1.0.0\n";
        let branches = parse_ls_remote_heads(output);
        assert_eq!(branches, vec!["main"]);
    }

    #[test]
    fn default_branch_prefers_main() {
        let branches = vec!["develop".into(), "main".into(), "master".into()];
        assert_eq!(default_branch(&branches).unwrap(), "main");
    }

    #[test]
    fn default_branch_falls_back_to_master() {
        let branches = vec!["develop".into(), "master".into()];
        assert_eq!(default_branch(&branches).unwrap(), "master");
    }

    #[test]
    fn default_branch_uses_first_when_no_preferred() {
        let branches = vec!["alpha".into(), "beta".into()];
        assert_eq!(default_branch(&branches).unwrap(), "alpha");
    }

    #[test]
    fn default_branch_empty_list() {
        assert!(default_branch(&[]).is_none());
    }

    #[test]
    fn validate_url_accepts_known_schemes() {
        assert!(validate_url("https://github.com/x/y.git").is_ok());
        assert!(validate_url("http://github.com/x/y.git").is_ok());
        assert!(validate_url("git@github.com:x/y.git").is_ok());
        assert!(validate_url("ssh://git@github.com/x/y.git").is_ok());
    }

    #[test]
    fn validate_url_rejects_unknown_schemes() {
        assert!(validate_url("ftp://example.com/repo").is_err());
        assert!(validate_url("file:///home/user/repo").is_err());
        assert!(validate_url("/absolute/path").is_err());
        assert!(validate_url("relative/path").is_err());
    }

    #[test]
    fn validate_branch_accepts_valid_names() {
        assert!(validate_branch("main").is_ok());
        assert!(validate_branch("feature/my-branch").is_ok());
        assert!(validate_branch("release_1.0").is_ok());
    }

    #[test]
    fn validate_branch_rejects_shell_chars() {
        assert!(validate_branch("branch; rm -rf /").is_err());
        assert!(validate_branch("branch$(cmd)").is_err());
        assert!(validate_branch("").is_err());
    }
}
