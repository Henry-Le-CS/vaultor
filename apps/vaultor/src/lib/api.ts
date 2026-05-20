/**
 * Typed wrappers around Tauri's invoke() IPC channel.
 */
import { invoke } from '@tauri-apps/api/core';
import { open as dialogOpen } from '@tauri-apps/plugin-dialog';

// ── Types ────────────────────────────────────────────────────

export interface SessionStatus {
  active: boolean;
  expires_at_ms: number | null;
}

// ── Namespace types ──────────────────────────────────────────

export interface Namespace {
  id: string;
  name: string;
  created_at: number;
  updated_at: number;
}

// ── Auth ────────────────────────────────────────────────────

/** Trigger TouchID, retrieve encryption key, create 2-minute session. */
export async function unlockVault(): Promise<SessionStatus> {
  return invoke<SessionStatus>('unlock_vault');
}

/** Manually lock the vault (wipes in-memory session + key). */
export async function lockVault(): Promise<void> {
  return invoke<void>('lock_vault');
}

/** Get current session status without side effects. */
export async function sessionStatus(): Promise<SessionStatus> {
  return invoke<SessionStatus>('session_status');
}

// ── Namespaces ───────────────────────────────────────────────

export async function listNamespaces(): Promise<Namespace[]> {
  return invoke<Namespace[]>('list_namespaces');
}

export async function createNamespace(name: string): Promise<Namespace> {
  return invoke<Namespace>('create_namespace', { name });
}

export async function renameNamespace(id: string, name: string): Promise<void> {
  return invoke<void>('rename_namespace', { id, name });
}

export async function deleteNamespace(id: string): Promise<void> {
  return invoke<void>('delete_namespace', { id });
}

// ── Secret types ─────────────────────────────────────────────

export interface SecretMeta {
  id: string;
  namespace_id: string;
  name: string;
  kind: 'kv' | 'file';
  is_draft: boolean;
  created_at: number;
  updated_at: number;
}

export interface KvFieldInput {
  id?: string;
  title: string;
  value: string;
  hidden: boolean;
}

export interface KvFieldDecrypted {
  id: string;
  title: string;
  value: string;
  hidden: boolean;
  sort_order: number;
}

// ── Secrets ──────────────────────────────────────────────────

export async function listSecrets(namespaceId: string): Promise<SecretMeta[]> {
  return invoke<SecretMeta[]>('list_secrets', { namespaceId });
}

export async function createKvSecret(
  namespaceId: string,
  name: string,
  fields: KvFieldInput[],
): Promise<SecretMeta> {
  return invoke<SecretMeta>('create_kv_secret', {
    input: { namespace_id: namespaceId, name, fields },
  });
}

export async function getKvSecret(id: string): Promise<KvFieldDecrypted[]> {
  return invoke<KvFieldDecrypted[]>('get_kv_secret', { id });
}

export async function updateKvSecret(
  id: string,
  name: string,
  fields: KvFieldInput[],
): Promise<void> {
  return invoke<void>('update_kv_secret', { input: { id, name, fields } });
}

export async function deleteSecret(id: string): Promise<void> {
  return invoke<void>('delete_secret', { id });
}

export async function saveKvDraft(
  namespaceId: string,
  name: string,
  fields: KvFieldInput[],
): Promise<SecretMeta> {
  return invoke<SecretMeta>('save_kv_draft', {
    input: { namespace_id: namespaceId, name, fields },
  });
}

export async function commitDraft(id: string): Promise<void> {
  return invoke<void>('commit_draft', { id });
}

export async function discardDraft(id: string): Promise<void> {
  return invoke<void>('discard_draft', { id });
}

// ── File secrets ─────────────────────────────────────────────

export interface FileSecretInfo {
  filename: string;
  size_bytes: number;
  content_b64: string;
}

export async function createFileSecret(
  namespaceId: string,
  name: string,
  filename: string,
  contentB64: string,
): Promise<SecretMeta> {
  return invoke<SecretMeta>('create_file_secret', {
    input: { namespace_id: namespaceId, name, filename, content_b64: contentB64 },
  });
}

export async function getFileSecret(id: string): Promise<FileSecretInfo> {
  return invoke<FileSecretInfo>('get_file_secret', { id });
}

export async function updateFileSecret(
  id: string,
  filename: string,
  contentB64: string,
): Promise<void> {
  return invoke<void>('update_file_secret', { id, filename, content_b64: contentB64 });
}

/** Delete all namespaces, secrets, and files from the local vault database. */
export async function clearLocalStorage(): Promise<void> {
  return invoke<void>('clear_local_storage');
}

// ── Password generator ──────────────────────────────────────────────────────

export interface PasswordOptions {
  length: number;
  useUppercase: boolean;
  useLowercase: boolean;
  useDigits: boolean;
  useSymbols: boolean;
}

export async function generatePassword(opts: PasswordOptions): Promise<string> {
  return invoke<string>('generate_password', {
    length: opts.length,
    useUppercase: opts.useUppercase,
    useLowercase: opts.useLowercase,
    useDigits: opts.useDigits,
    useSymbols: opts.useSymbols,
  });
}

// ── Settings ─────────────────────────────────────────────────────────────────

export type SessionExpiry = 'minutes_2' | 'minutes_5' | 'minutes_10' | 'until_quit';

export interface GitRemoteInfo {
  id: string;
  url: string;
  branch: string;
  last_synced: number | null;
}

export interface AppSettings {
  session_expiry: SessionExpiry;
  db_path: string;
  /** Active git remote, or null if in local SQLite mode. */
  git_remote: GitRemoteInfo | null;
  /** All configured git repositories. */
  git_remotes: GitRemoteInfo[];
  /** Whether the user has completed the onboarding tutorial. */
  tutorial_seen: boolean;
}

export async function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>('get_settings');
}

export async function setSessionExpiry(expiry: SessionExpiry): Promise<void> {
  return invoke<void>('set_session_expiry', { expiry });
}

/** Mark the onboarding tutorial as completed. */
export async function setTutorialSeen(): Promise<void> {
  return invoke<void>('set_tutorial_seen');
}

export async function getStorageLocation(): Promise<string> {
  return invoke<string>('get_storage_location');
}

/** Open a native folder picker.  Returns the chosen path or null if cancelled. */
export async function pickFolder(): Promise<string | null> {
  const result = await dialogOpen({ directory: true, multiple: false });
  if (!result) return null;
  return Array.isArray(result) ? result[0] : result;
}

/**
 * Copy the vault DB to `newDir/vaultor.db`, verify integrity, update settings.
 *
 * @param force - if true, overwrite an existing `vaultor.db` at the destination.
 * @returns the new path string on success.
 * @throws `"destination_exists"` if destination has vaultor.db and force is false.
 */
export async function moveStorage(newDir: string, force = false): Promise<string> {
  return invoke<string>('move_storage', { newDir, force });
}

// ── Git remote storage ────────────────────────────────────────────────────────

export interface GitConnectionResult {
  branches: string[];
  default_branch: string;
}

export interface GitStatus {
  connected: boolean;
  url: string | null;
  branch: string | null;
  last_synced: number | null;
}

/** Test connectivity to a remote git repository and return available branches. */
export async function testGitConnection(url: string): Promise<GitConnectionResult> {
  return invoke<GitConnectionResult>('test_git_connection', { url });
}

/** Clone the remote, open an isolated git DB, and switch the active environment. */
export async function connectGitRemote(url: string, branch: string): Promise<void> {
  return invoke<void>('connect_git_remote', { url, branch });
}

/** Return the current git remote connection status from settings. */
export async function getGitStatus(): Promise<GitStatus> {
  return invoke<GitStatus>('get_git_status');
}

/** Switch the active git remote to a different repository by URL. */
export async function switchGitRemote(url: string): Promise<void> {
  return invoke<void>('switch_git_remote', { url });
}

/** Switch back to local SQLite mode (git remote stays in the configured list). */
export async function disconnectGitRemote(): Promise<void> {
  return invoke<void>('disconnect_git_remote');
}

export interface SyncResult {
  namespaces_synced: number;
  secrets_synced: number;
  committed: boolean;
  pushed: boolean;
  pulled: boolean;
}

/** Run a full pull → merge → push sync cycle against the active git remote. */
export async function syncGit(): Promise<SyncResult> {
  return invoke<SyncResult>('sync_git');
}

export interface RemoveGitRemoteResult {
  new_active_url: string | null;
}

/** Remove a configured git repository and delete its local clone. */
export async function removeGitRemote(url: string): Promise<RemoveGitRemoteResult> {
  return invoke<RemoveGitRemoteResult>('remove_git_remote', { url });
}
