<script lang="ts">
  import { activeNamespaceId, namespaces } from '../../stores/namespace.js';
  import { secrets } from '../../stores/secrets.js';
  import { listSecrets, deleteSecret } from '../../api.js';
  import ConfirmModal from '../shared/ConfirmModal.svelte';

  interface Props {
    activeSecretId: string;
    onCreateNew: () => void;
    onSecretDeleted: (id: string) => void;
  }

  let {
    activeSecretId = $bindable(''),
    onCreateNew,
    onSecretDeleted,
  }: Props = $props();

  let loading = $state(false);

  // ── Delete confirm state ──────────────────────────────────
  let deletingId = $state<string | null>(null);
  const deletingSecret = $derived($secrets.find((s) => s.id === deletingId));

  // Reload when active namespace changes; clear active secret so editors close.
  $effect(() => {
    const nsId = $activeNamespaceId;
    activeSecretId = '';
    if (nsId) {
      loadSecrets(nsId);
    } else {
      secrets.set([]);
    }
  });

  async function loadSecrets(namespaceId: string) {
    loading = true;
    try {
      const list = await listSecrets(namespaceId);
      secrets.set(list);
    } catch (err) {
      console.error('Failed to load secrets', err);
    } finally {
      loading = false;
    }
  }

  function handleDelete(id: string) {
    deletingId = id;
  }

  async function confirmDelete() {
    const id = deletingId;
    deletingId = null;
    if (!id) return;
    try {
      await deleteSecret(id);
      secrets.update((list) => list.filter((x) => x.id !== id));
      if (activeSecretId === id) {
        activeSecretId = '';
        onSecretDeleted(id);
      }
    } catch (err) {
      console.error('delete secret failed', err);
    }
  }
</script>

<section class="secrets-list" aria-label="Secrets">
  <header class="list-header">
    <div class="list-title-group">
      {#if $activeNamespaceId}
        {@const ns = $namespaces.find((n) => n.id === $activeNamespaceId)}
        <span class="list-ns-name">{ns?.name ?? ''}</span>
      {/if}
      <span class="list-title">Secrets</span>
    </div>
    <button class="new-btn" onclick={onCreateNew} disabled={!$activeNamespaceId}>
      <svg viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
        <line x1="10" y1="4" x2="10" y2="16" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
        <line x1="4" y1="10" x2="16" y2="10" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
      </svg>
      New
    </button>
  </header>

  <div class="list-body">
    {#if !$activeNamespaceId}
      <div class="empty-state">
        <svg viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
          <rect x="4" y="10" width="40" height="30" rx="4" stroke="var(--border)" stroke-width="2"/>
          <path d="M4 18h40" stroke="var(--border)" stroke-width="2"/>
          <path d="M4 14l8-8h8l-8 8" stroke="var(--border)" stroke-width="2" stroke-linejoin="round"/>
        </svg>
        <p>No namespace</p>
        <span>Use the <strong>+</strong> button on the left to create one.</span>
      </div>
    {:else if loading}
      <div class="empty-state"><p>Loading…</p></div>
    {:else if $secrets.length === 0}
      <div class="empty-state">
        <svg viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
          <rect x="8" y="14" width="32" height="26" rx="4" stroke="var(--border)" stroke-width="2"/>
          <path d="M16 14v-4a8 8 0 0 1 16 0v4" stroke="var(--border)" stroke-width="2" stroke-linecap="round"/>
          <circle cx="24" cy="27" r="3" fill="var(--border)"/>
        </svg>
        <p>No secrets yet</p>
        <span>Click <strong>New</strong> to add one.</span>
      </div>
    {:else}
      {#each $secrets as s (s.id)}
        <div
          class="secret-row"
          class:active={activeSecretId === s.id}
          class:draft={s.is_draft}
          role="button"
          tabindex="0"
          onclick={() => (activeSecretId = s.id)}
          onkeydown={(e) => e.key === 'Enter' && (activeSecretId = s.id)}
        >
          <!-- Kind icon -->
          <span class="kind-icon" aria-hidden="true">
            {#if s.kind === 'file'}
              <!-- Document with fold + two content lines -->
              <svg viewBox="0 0 16 16" fill="none">
                <path d="M3 1.5h6.5L13 5v9.5H3V1.5Z" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
                <path d="M9.5 1.5V5H13" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                <path d="M5.5 8h5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
                <path d="M5.5 10.5h3.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
              </svg>
            {:else}
              <!-- Bullet list — represents key-value entries -->
              <svg viewBox="0 0 16 16" fill="none">
                <circle cx="3" cy="4.5" r="1" fill="currentColor"/>
                <path d="M6 4.5h7" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
                <circle cx="3" cy="8" r="1" fill="currentColor"/>
                <path d="M6 8h7" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
                <circle cx="3" cy="11.5" r="1" fill="currentColor"/>
                <path d="M6 11.5h7" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
              </svg>
            {/if}
          </span>
          <span class="secret-name">{s.name}</span>
          {#if s.is_draft}
            <span class="draft-dot" title="Unsaved draft" aria-label="Draft"></span>
          {/if}
          <!-- Inline delete button — visible on row hover -->
          <button
            class="row-delete-btn"
            title="Delete secret"
            aria-label="Delete {s.name}"
            onclick={(e) => { e.stopPropagation(); handleDelete(s.id); }}
          >
            <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path d="M3 4.5h10M6 4.5v-1a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1v1"
                stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
              <path d="M5 4.5l.5 8h5l.5-8"
                stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </button>
        </div>
      {/each}
    {/if}
  </div>
</section>

{#if deletingId !== null && deletingSecret}
  <ConfirmModal
    title="Delete secret"
    message='Delete "{deletingSecret.name}"? This cannot be undone.'
    confirmLabel="Delete"
    onConfirm={confirmDelete}
    onCancel={() => (deletingId = null)}
  />
{/if}

<style>
  .secrets-list {
    width: var(--list-width);
    min-width: 180px;
    height: 100%;
    background: var(--bg-a);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    overflow: hidden;
  }

  .list-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 14px 10px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .list-title-group {
    display: flex;
    flex-direction: column;
    gap: 1px;
    overflow: hidden;
  }

  .list-ns-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .list-title {
    font-weight: 600;
    font-size: 10px;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .new-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 10px;
    background: var(--brand-dark);
    color: white;
    border-radius: var(--radius-sm);
    font-size: 13px;
    font-weight: 500;
    transition: background 0.15s;
  }
  .new-btn:hover:not(:disabled) { background: var(--brand-mid); }
  .new-btn:disabled { opacity: 0.5; cursor: default; }
  .new-btn svg { width: 14px; height: 14px; }

  .list-body {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--muted);
    padding: 32px 16px;
    text-align: center;
  }
  .empty-state svg { width: 48px; height: 48px; opacity: 0.5; }
  .empty-state p { font-size: 14px; font-weight: 500; }
  .empty-state span { font-size: 12px; color: var(--muted); }

  .secret-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    cursor: pointer;
    border-bottom: 1px solid var(--border);
    transition: background 0.1s;
    position: relative;
  }
  .secret-row:hover { background: var(--bg-b); }
  .secret-row.active { background: var(--brand); }
  .secret-row.draft { background: var(--err-bg); }
  .secret-row.draft.active { background: color-mix(in srgb, var(--err-bg) 60%, var(--brand) 40%); }

  .kind-icon svg { width: 14px; height: 14px; color: var(--muted); flex-shrink: 0; }

  .secret-name {
    flex: 1;
    font-size: 13px;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .draft-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--err);
    flex-shrink: 0;
  }

  /* ── Inline delete button ─────────────────────────────── */
  .row-delete-btn {
    flex-shrink: 0;
    width: 26px;
    height: 26px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    opacity: 0;
    transition: opacity 0.1s, background 0.1s, color 0.1s;
  }

  .row-delete-btn svg { width: 13px; height: 13px; }

  /* Reveal on row hover */
  .secret-row:hover .row-delete-btn { opacity: 1; }
  .row-delete-btn:hover { background: var(--err-bg); color: var(--err); }
</style>
