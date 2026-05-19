<script lang="ts">
  import { type GitRemoteInfo } from '../../api.js';

  interface Props {
    remotes: GitRemoteInfo[];
    activeUrl: string | null;
    onSwitch: (url: string) => Promise<void>;
    onOpenSettings: () => void;
  }
  let { remotes, activeUrl, onSwitch, onOpenSettings }: Props = $props();

  const ADD_SENTINEL = '__add__';

  // ── Switch state ──────────────────────────────────────────────────────────
  let switching = $state(false);
  let switchErr = $state('');

  const activeRemote = $derived(remotes.find((r) => r.url === activeUrl) ?? null);

  function formatLastSynced(ts: number | null): string {
    if (ts === null) return 'Never synced';
    const diff = Math.floor((Date.now() - ts) / 1000);
    if (diff < 5) return 'Just now';
    if (diff < 60) return `${diff}s ago`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    return new Date(ts).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }

  function truncateUrl(u: string, max = 38): string {
    // Show only the path part if it's a standard git URL.
    const m = u.match(/[:/]([^/:]+\/[^/:]+?)(?:\.git)?$/);
    const label = m ? m[1] : u;
    return label.length > max ? label.slice(0, max) + '…' : label;
  }

  function onSelectChange(e: Event) {
    const val = (e.target as HTMLSelectElement).value;
    if (val === ADD_SENTINEL) {
      (e.target as HTMLSelectElement).value = activeUrl ?? '';
      onOpenSettings();
      return;
    }
    if (val !== activeUrl) {
      switching = true;
      switchErr = '';
      onSwitch(val)
        .catch((err: unknown) => {
          switchErr = err instanceof Error ? err.message : String(err);
          setTimeout(() => (switchErr = ''), 5000);
          (e.target as HTMLSelectElement).value = activeUrl ?? '';
        })
        .finally(() => {
          switching = false;
        });
    }
  }
</script>

<div class="git-bar">
  <!-- Repo icon + select -->
  <div class="bar-left">
    <svg class="git-icon" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="4" cy="4" r="1.5" stroke="currentColor" stroke-width="1.2"/>
      <circle cx="4" cy="12" r="1.5" stroke="currentColor" stroke-width="1.2"/>
      <circle cx="12" cy="4" r="1.5" stroke="currentColor" stroke-width="1.2"/>
      <path d="M4 5.5v5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
      <path d="M5.5 4h3a3 3 0 0 1 3 3v.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
    </svg>

    <select
      class="repo-select"
      value={activeUrl ?? ''}
      onchange={onSelectChange}
      disabled={switching}
      title={activeUrl ?? ''}
      aria-label="Active git repository"
    >
      {#each remotes as r (r.url)}
        <option value={r.url}>{truncateUrl(r.url)} [{r.branch}]</option>
      {/each}
      <option disabled>──────────────</option>
      <option value={ADD_SENTINEL}>+ Add repository…</option>
    </select>
  </div>

  <!-- Right side: status -->
  <div class="bar-right">
    {#if switching}
      <span class="sync-ok">Switching…</span>
    {:else if switchErr}
      <span class="sync-err" title={switchErr}>Switch failed</span>
    {:else if activeRemote}
      <span class="last-synced">{formatLastSynced(activeRemote.last_synced)}</span>
    {/if}
  </div>
</div>

<style>
  .git-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 10px;
    background: var(--bg-b);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    min-width: 0;
  }

  /* ── Left ────────────────────────────────────────────────── */
  .bar-left {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    flex: 1;
  }

  .git-icon {
    width: 13px;
    height: 13px;
    color: var(--muted);
    flex-shrink: 0;
  }

  .repo-select {
    flex: 1;
    min-width: 0;
    height: 26px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--card);
    color: var(--text);
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, monospace;
    padding: 0 6px;
    cursor: pointer;
  }

  .repo-select:focus {
    outline: none;
    border-color: var(--brand-dark);
  }

  /* ── Right ───────────────────────────────────────────────── */
  .bar-right {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .last-synced {
    font-size: 10px;
    color: var(--muted);
    white-space: nowrap;
  }

  .sync-ok {
    font-size: 10px;
    color: #22c55e;
    white-space: nowrap;
  }

  .sync-err {
    font-size: 10px;
    color: var(--err);
    white-space: nowrap;
    cursor: help;
  }

</style>
