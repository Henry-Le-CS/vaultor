<script lang="ts">
  import { onMount } from 'svelte';
  import { namespaces, activeNamespaceId } from '../../stores/namespace.js';
  import {
    listNamespaces,
    createNamespace,
    renameNamespace,
    deleteNamespace,
  } from '../../api.js';
  import SettingsModal from '../settings/SettingsModal.svelte';
  import ConfirmModal from '../shared/ConfirmModal.svelte';

  interface Props { showLabels?: boolean; }
  let { showLabels = false }: Props = $props();

  // ── Settings modal ────────────────────────────────────────
  let settingsOpen = $state(false);

  // ── Delete confirm ────────────────────────────────────────
  let deletingId = $state<string | null>(null);
  const deletingNs = $derived($namespaces.find((n) => n.id === deletingId));

  // ── Rename ────────────────────────────────────────────────
  let renamingId = $state<string | null>(null);
  let renameValue = $state('');
  let renameInput = $state<HTMLInputElement | null>(null);

  // ── Create ────────────────────────────────────────────────
  let creating = $state(false);
  let newNsValue = $state('');
  let newNsInput = $state<HTMLInputElement | null>(null);

  // ── Hover tooltip (narrow mode only) ─────────────────────
  // position: fixed tooltip that appears to the right of the hovered item.
  // A short hide-delay lets the mouse travel from item → tooltip without
  // the tooltip blinking away.
  let tooltipNsId = $state<string | null>(null);
  let tooltipTop = $state(0);
  let tooltipLeft = $state(0);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;

  function showTooltip(e: MouseEvent, id: string) {
    if (showLabels) return; // wide mode uses inline buttons
    if (hideTimer) { clearTimeout(hideTimer); hideTimer = null; }
    tooltipNsId = id;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    tooltipTop = rect.top + rect.height / 2;
    tooltipLeft = rect.right + 6;
  }

  function scheduleHide() {
    hideTimer = setTimeout(() => { tooltipNsId = null; }, 120);
  }

  function cancelHide() {
    if (hideTimer) { clearTimeout(hideTimer); hideTimer = null; }
  }

  // Clear tooltip when sidebar expands to wide mode
  $effect(() => { if (showLabels) tooltipNsId = null; });

  // ── Load ──────────────────────────────────────────────────
  onMount(async () => {
    try {
      const list = await listNamespaces();
      namespaces.set(list);
      if (list.length > 0 && !$activeNamespaceId) {
        activeNamespaceId.set(list[0].id);
      }
    } catch (err) {
      console.error('Failed to load namespaces', err);
    }
  });

  // ── Handlers ─────────────────────────────────────────────
  function handleAdd() {
    creating = true;
    newNsValue = '';
    setTimeout(() => newNsInput?.focus(), 0);
  }

  async function commitCreate() {
    const trimmed = newNsValue.trim();
    creating = false;
    newNsValue = '';
    if (!trimmed) return;
    try {
      const ns = await createNamespace(trimmed);
      namespaces.update((list) => [...list, ns]);
      activeNamespaceId.set(ns.id);
    } catch (err) {
      console.error('create namespace failed', err);
    }
  }

  function startRename(id: string, currentName: string) {
    tooltipNsId = null;
    renamingId = id;
    renameValue = currentName;
    setTimeout(() => renameInput?.select(), 0);
  }

  async function commitRename(id: string) {
    const trimmed = renameValue.trim();
    renamingId = null;
    if (!trimmed) return;
    try {
      await renameNamespace(id, trimmed);
      namespaces.update((list) =>
        list.map((ns) => (ns.id === id ? { ...ns, name: trimmed } : ns)),
      );
    } catch (err) {
      console.error('rename failed', err);
    }
  }

  function handleDelete(id: string) {
    tooltipNsId = null;
    deletingId = id;
  }

  async function confirmDelete() {
    const id = deletingId;
    deletingId = null;
    if (!id) return;
    try {
      await deleteNamespace(id);
      namespaces.update((list) => list.filter((n) => n.id !== id));
      if ($activeNamespaceId === id) {
        const remaining = $namespaces.filter((n) => n.id !== id);
        activeNamespaceId.set(remaining[0]?.id ?? '');
      }
    } catch (err) {
      console.error('delete failed', err);
    }
  }
</script>

<aside class="ns-sidebar" aria-label="Namespaces">
  <!-- Gear icon -->
  <div class="ns-top">
    <button
      class="ns-gear"
      title="Settings"
      aria-label="Open settings"
      onclick={() => (settingsOpen = true)}
    >
      <svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" stroke="currentColor" stroke-width="1.5"
          d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0 1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94H9.75c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z"/>
        <path stroke-linecap="round" stroke-linejoin="round" stroke="currentColor" stroke-width="1.5"
          d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"/>
      </svg>
    </button>
  </div>

  <div class="ns-list">
    {#each $namespaces as ns (ns.id)}
      <div
        class="ns-item-wrap"
        class:ns-item-wrap--wide={showLabels}
        onmouseenter={(e) => showTooltip(e, ns.id)}
        onmouseleave={scheduleHide}
        role="none"
      >
        {#if renamingId === ns.id}
          <input
            bind:this={renameInput}
            class="ns-rename-input"
            class:ns-rename-input--wide={showLabels}
            bind:value={renameValue}
            onblur={() => commitRename(ns.id)}
            onkeydown={(e) => {
              if (e.key === 'Enter') commitRename(ns.id);
              if (e.key === 'Escape') (renamingId = null);
            }}
            aria-label="Rename namespace"
          />
        {:else}
          <button
            class="ns-item"
            class:active={$activeNamespaceId === ns.id}
            class:ns-item--wide={showLabels}
            onclick={() => activeNamespaceId.set(ns.id)}
            title={ns.name}
            aria-label={ns.name}
            aria-pressed={$activeNamespaceId === ns.id}
          >
            <span class="ns-initial">{ns.name[0].toUpperCase()}</span>
            {#if showLabels}
              <span class="ns-label">{ns.name}</span>
            {/if}
          </button>

          <!-- Wide mode: inline action buttons, always visible -->
          {#if showLabels}
            <div class="ns-actions-inline">
              <button
                class="ns-action-btn"
                title="Rename"
                aria-label="Rename {ns.name}"
                onclick={(e) => { e.stopPropagation(); startRename(ns.id, ns.name); }}
              >
                <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <path d="M11.5 2.5a1.5 1.5 0 0 1 2 2L5 13H3v-2L11.5 2.5Z"
                    stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
                </svg>
              </button>
              <button
                class="ns-action-btn ns-action-btn--danger"
                title="Delete"
                aria-label="Delete {ns.name}"
                onclick={(e) => { e.stopPropagation(); handleDelete(ns.id); }}
              >
                <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <path d="M3 4.5h10M6 4.5v-1a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1v1"
                    stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                  <path d="M5 4.5l.5 8h5l.5-8"
                    stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </button>
            </div>
          {/if}
        {/if}
      </div>
    {/each}

    {#if creating}
      <div class="ns-item-wrap">
        <input
          bind:this={newNsInput}
          class="ns-rename-input"
          bind:value={newNsValue}
          placeholder="Name…"
          onblur={commitCreate}
          onkeydown={(e) => {
            if (e.key === 'Enter') commitCreate();
            if (e.key === 'Escape') { creating = false; newNsValue = ''; }
          }}
          aria-label="New namespace name"
        />
      </div>
    {/if}
  </div>

  {#if $namespaces.length === 0 && !creating}
    <div class="ns-empty-hint" aria-hidden="true">↓</div>
  {/if}

  <div class="ns-footer">
    <button
      class="ns-add"
      class:ns-add--prominent={$namespaces.length === 0}
      title="New namespace"
      aria-label="New namespace"
      onclick={handleAdd}
    >
      <svg viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
        <line x1="10" y1="4" x2="10" y2="16" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
        <line x1="4" y1="10" x2="16" y2="10" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
      </svg>
    </button>
  </div>
</aside>

<!-- Narrow-mode tooltip: position:fixed so it escapes overflow clipping.
     Rendered outside <aside> so it's never clipped by the sidebar's scroll container. -->
{#if tooltipNsId !== null}
  {@const tooltipNs = $namespaces.find((n) => n.id === tooltipNsId)}
  {#if tooltipNs && !showLabels}
    <div
      class="ns-tooltip"
      style="top:{tooltipTop}px; left:{tooltipLeft}px"
      onmouseenter={cancelHide}
      onmouseleave={() => { tooltipNsId = null; }}
      role="toolbar"
      aria-label="Namespace actions"
      tabindex="-1"
    >
      <button
        class="ns-tooltip-btn"
        title="Rename"
        aria-label="Rename {tooltipNs.name}"
        onclick={() => startRename(tooltipNs.id, tooltipNs.name)}
      >
        <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <path d="M11.5 2.5a1.5 1.5 0 0 1 2 2L5 13H3v-2L11.5 2.5Z"
            stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
        </svg>
        <span>Rename</span>
      </button>
      <button
        class="ns-tooltip-btn ns-tooltip-btn--danger"
        title="Delete"
        aria-label="Delete {tooltipNs.name}"
        onclick={() => handleDelete(tooltipNs.id)}
      >
        <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <path d="M3 4.5h10M6 4.5v-1a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1v1"
            stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
          <path d="M5 4.5l.5 8h5l.5-8"
            stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <span>Delete</span>
      </button>
    </div>
  {/if}
{/if}

<SettingsModal open={settingsOpen} onClose={() => (settingsOpen = false)} />

{#if deletingId !== null && deletingNs}
  <ConfirmModal
    title="Delete namespace"
    message='All secrets inside "{deletingNs.name}" will be permanently removed. This cannot be undone.'
    confirmLabel="Delete namespace"
    requiredPhrase="delete {deletingNs.name}"
    onConfirm={confirmDelete}
    onCancel={() => (deletingId = null)}
  />
{/if}

<style>
  .ns-sidebar {
    width: var(--sidebar-width);
    min-width: var(--sidebar-width);
    height: 100%;
    background: var(--bg-b);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 8px 0;
    flex-shrink: 0;
  }

  /* ── Gear ────────────────────────────────────────────────── */
  .ns-top {
    width: 100%;
    display: flex;
    justify-content: center;
    padding: 0 8px 6px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 6px;
  }

  .ns-gear {
    width: 36px;
    height: 36px;
    border-radius: var(--radius);
    border: none;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    transition: background 0.15s, color 0.15s;
  }

  .ns-gear:hover { background: var(--bg-a); color: var(--brand-dark); }
  .ns-gear svg { width: 18px; height: 18px; }

  /* ── List ────────────────────────────────────────────────── */
  .ns-list {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    overflow-y: auto;
    width: 100%;
  }

  /* ── Item wrapper ────────────────────────────────────────── */
  .ns-item-wrap {
    width: 100%;
    display: flex;
    justify-content: center;
  }

  /* Wide: flex row so inline action buttons sit to the right */
  .ns-item-wrap--wide {
    justify-content: flex-start;
    align-items: center;
    gap: 4px;
    padding: 0 4px;
  }

  /* ── Namespace button ────────────────────────────────────── */
  .ns-item {
    width: 40px;
    height: 40px;
    border-radius: var(--radius);
    background: var(--card);
    border: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    font-weight: 600;
    font-size: 15px;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
    flex-shrink: 0;
  }

  .ns-item:hover { background: var(--brand); color: var(--text); }

  .ns-item.active {
    background: var(--brand-dark);
    color: white;
    border-color: var(--brand-dark);
  }

  .ns-initial { line-height: 1; user-select: none; flex-shrink: 0; }

  .ns-item--wide {
    flex: 1;
    width: auto;
    justify-content: flex-start;
    gap: 8px;
    padding: 0 10px;
  }

  .ns-label {
    flex: 1;
    font-size: 12px;
    font-weight: 500;
    color: inherit;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }

  /* ── Rename input ────────────────────────────────────────── */
  .ns-rename-input {
    width: 40px;
    height: 40px;
    border-radius: var(--radius);
    border: 2px solid var(--brand-dark);
    background: var(--card);
    color: var(--text);
    font-size: 12px;
    text-align: center;
    padding: 2px;
  }

  .ns-rename-input--wide {
    flex: 1;
    width: auto;
    text-align: left;
    padding: 0 10px;
  }

  /* ── Wide-mode inline action buttons ────────────────────── */
  .ns-actions-inline {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }

  .ns-action-btn {
    width: 26px;
    height: 26px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    transition: background 0.1s, color 0.1s;
  }

  .ns-action-btn:hover { background: var(--bg-b); color: var(--text); }
  .ns-action-btn--danger:hover { background: var(--err-bg); color: var(--err); }
  .ns-action-btn svg { width: 13px; height: 13px; }

  /* ── Narrow-mode fixed tooltip ───────────────────────────── */
  .ns-tooltip {
    position: fixed;
    transform: translateY(-50%);
    z-index: 600;
    display: flex;
    flex-direction: column;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 4px 16px var(--shadow);
    overflow: hidden;
    min-width: 108px;
  }

  .ns-tooltip-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    font-size: 13px;
    color: var(--text);
    transition: background 0.1s;
    white-space: nowrap;
  }

  .ns-tooltip-btn + .ns-tooltip-btn {
    border-top: 1px solid var(--border);
  }

  .ns-tooltip-btn:hover { background: var(--bg-b); }
  .ns-tooltip-btn--danger { color: var(--err); }
  .ns-tooltip-btn--danger:hover { background: var(--err-bg); }
  .ns-tooltip-btn svg { width: 14px; height: 14px; flex-shrink: 0; }

  /* ── Footer ──────────────────────────────────────────────── */
  .ns-footer {
    padding: 8px;
    width: 100%;
    display: flex;
    justify-content: center;
  }

  .ns-add {
    width: 36px;
    height: 36px;
    border-radius: var(--radius);
    border: 1.5px dashed var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    transition: border-color 0.15s, color 0.15s;
  }

  .ns-add:hover { border-color: var(--brand-dark); color: var(--brand-dark); }
  .ns-add svg { width: 18px; height: 18px; }

  .ns-add--prominent {
    border-style: solid;
    border-color: var(--brand-dark);
    background: var(--brand-dark);
    color: white;
    animation: pulse 2s ease-in-out infinite;
  }
  .ns-add--prominent:hover { background: var(--brand-mid); border-color: var(--brand-mid); }

  @keyframes pulse {
    0%, 100% { box-shadow: 0 0 0 0 color-mix(in srgb, var(--brand-dark) 40%, transparent); }
    50%       { box-shadow: 0 0 0 5px color-mix(in srgb, var(--brand-dark) 0%, transparent); }
  }

  .ns-empty-hint {
    font-size: 16px;
    color: var(--brand-dark);
    opacity: 0.6;
    animation: bounce 1.2s ease-in-out infinite;
    user-select: none;
  }

  @keyframes bounce {
    0%, 100% { transform: translateY(0); }
    50%       { transform: translateY(4px); }
  }
</style>
