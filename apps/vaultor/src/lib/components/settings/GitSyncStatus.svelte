<script lang="ts">
  import { syncGit, disconnectGitRemote, type SyncResult } from '../../api.js';

  interface Props {
    url: string;
    branch: string;
    lastSynced: number | null;
    onDisconnected: () => void;
  }
  let { url, branch, lastSynced = $bindable(), onDisconnected }: Props = $props();

  // ── Sync state ────────────────────────────────────────────────────────────
  let syncing = $state(false);
  let syncError = $state('');
  let syncMessage = $state('');

  // ── Disconnect state ──────────────────────────────────────────────────────
  let disconnecting = $state(false);
  let disconnectError = $state('');
  let confirmDisconnect = $state(false);

  // ── Helpers ───────────────────────────────────────────────────────────────
  function formatLastSynced(ts: number | null): string {
    if (ts === null) return 'Never';
    const diff = Math.floor((Date.now() - ts) / 1000);
    if (diff < 5)   return 'Just now';
    if (diff < 60)  return `${diff}s ago`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    return new Date(ts).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }

  function truncateUrl(u: string, max = 42): string {
    return u.length > max ? u.slice(0, max) + '…' : u;
  }

  // ── Actions ───────────────────────────────────────────────────────────────
  async function handleSync() {
    syncing = true;
    syncError = '';
    syncMessage = '';
    try {
      const result: SyncResult = await syncGit();
      lastSynced = Date.now();
      if (!result.committed) {
        syncMessage = 'Already up to date.';
      } else {
        syncMessage = `Synced ${result.secrets_synced} secret${result.secrets_synced !== 1 ? 's' : ''}, ${result.namespaces_synced} namespace${result.namespaces_synced !== 1 ? 's' : ''}.`;
      }
    } catch (e: unknown) {
      syncError = e instanceof Error ? e.message : String(e);
    } finally {
      syncing = false;
    }
  }

  async function handleDisconnect() {
    disconnecting = true;
    disconnectError = '';
    try {
      const result = await disconnectGitRemote();
      if (result.offline) {
        // Disconnected but couldn't pull final state — inform user and continue.
        disconnectError = 'Disconnected (offline — remote may have newer data).';
        // Still call onDisconnected after a moment so UI updates.
        setTimeout(onDisconnected, 1800);
      } else {
        onDisconnected();
      }
    } catch (e: unknown) {
      disconnectError = e instanceof Error ? e.message : String(e);
      disconnecting = false;
    }
  }
</script>

<div class="sync-status">
  <!-- Mode badge -->
  <span class="mode-badge mode-badge--git">Git storage</span>

  <!-- Connection info -->
  <div class="info-grid">
    <span class="info-label">Repository</span>
    <span class="info-value mono" title={url}>{truncateUrl(url)}</span>

    <span class="info-label">Branch</span>
    <span class="info-value mono">{branch}</span>

    <span class="info-label">Last synced</span>
    <span class="info-value">{formatLastSynced(lastSynced)}</span>
  </div>

  <!-- Sync result / error -->
  {#if syncMessage}
    <p class="sync-ok" role="status">{syncMessage}</p>
  {/if}
  {#if syncError}
    <p class="sync-error" role="alert">{syncError}</p>
  {/if}

  <!-- Action buttons -->
  <div class="actions">
    <button class="primary-btn" onclick={handleSync} disabled={syncing || disconnecting}>
      {syncing ? 'Syncing…' : 'Sync Now'}
    </button>

    {#if !confirmDisconnect}
      <button
        class="ghost-btn"
        onclick={() => { confirmDisconnect = true; syncError = ''; syncMessage = ''; }}
        disabled={syncing || disconnecting}
      >
        Switch to Local…
      </button>
    {:else}
      <span class="confirm-text">Switch back to local SQLite?</span>
      <button
        class="danger-btn"
        onclick={handleDisconnect}
        disabled={disconnecting}
      >
        {disconnecting ? 'Switching…' : 'Yes, switch to local'}
      </button>
      <button
        class="ghost-btn"
        onclick={() => { confirmDisconnect = false; disconnectError = ''; }}
        disabled={disconnecting}
      >
        Cancel
      </button>
    {/if}
  </div>

  {#if disconnectError}
    <p class="sync-error" role="alert">{disconnectError}</p>
  {/if}
</div>

<style>
  .sync-status {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  /* ── Mode badge (matches .mode-badge in SettingsModal) ───────── */
  .mode-badge {
    display: inline-flex;
    align-items: center;
    height: 22px;
    padding: 0 8px;
    border-radius: 99px;
    border: 1px solid var(--brand-dark);
    font-size: 11px;
    font-weight: 600;
    color: var(--brand-dark);
    background: color-mix(in srgb, var(--brand-dark) 10%, var(--bg-b));
    width: fit-content;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  /* ── Info grid ───────────────────────────────────────────────── */
  .info-grid {
    display: grid;
    grid-template-columns: 72px 1fr;
    gap: 4px 10px;
    align-items: baseline;
  }

  .info-label {
    font-size: 12px;
    color: var(--muted);
  }

  .info-value {
    font-size: 12px;
    color: var(--text);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .info-value.mono {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 11px;
    color: var(--muted);
  }

  /* ── Messages ────────────────────────────────────────────────── */
  .sync-ok {
    font-size: 12px;
    color: #22c55e;
    margin: 0;
  }

  .sync-error {
    font-size: 12px;
    color: var(--err);
    margin: 0;
  }

  /* ── Actions ─────────────────────────────────────────────────── */
  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }

  .confirm-text {
    font-size: 12px;
    color: var(--text);
  }

  .primary-btn {
    height: 32px;
    padding: 0 14px;
    background: var(--brand-dark);
    color: white;
    border-radius: var(--radius-sm);
    font-size: 13px;
    font-weight: 500;
    transition: background 0.1s;
  }

  .primary-btn:hover:not(:disabled) { background: var(--brand-mid); }
  .primary-btn:disabled { opacity: 0.5; cursor: default; }

  .ghost-btn {
    height: 32px;
    padding: 0 12px;
    border-radius: var(--radius-sm);
    color: var(--muted);
    font-size: 13px;
    transition: color 0.1s;
  }

  .ghost-btn:hover:not(:disabled) { color: var(--text); }
  .ghost-btn:disabled { opacity: 0.5; cursor: default; }

  .danger-btn {
    height: 32px;
    padding: 0 12px;
    background: var(--err);
    color: white;
    border-radius: var(--radius-sm);
    font-size: 13px;
    font-weight: 500;
    transition: opacity 0.1s;
  }

  .danger-btn:hover:not(:disabled) { opacity: 0.85; }
  .danger-btn:disabled { opacity: 0.5; cursor: default; }
</style>
