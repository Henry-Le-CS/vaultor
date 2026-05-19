<script lang="ts">
  import { onMount } from 'svelte';
  import NamespaceSidebar from './NamespaceSidebar.svelte';
  import SecretsList from './SecretsList.svelte';
  import SecretDetail from './SecretDetail.svelte';
  import SettingsModal from '../settings/SettingsModal.svelte';
  import OnboardingTutorial from '../shared/OnboardingTutorial.svelte';
  import { getSettings, switchGitRemote, syncGit, listNamespaces, type GitRemoteInfo } from '../../api.js';
  import { namespaces, activeNamespaceId } from '../../stores/namespace.js';

  let activeSecretId = $state('');
  let creatingNew = $state(false);

  function handleCreateNew() {
    creatingNew = true;
    activeSecretId = '';
  }

  function handleSecretSaved(id: string) {
    activeSecretId = id;
    creatingNew = false;
  }

  function handleCancelled() {
    creatingNew = false;
  }

  function handleSecretDeleted(id: string) {
    if (activeSecretId === id) activeSecretId = '';
  }

  // ── Onboarding tutorial ────────────────────────────────────────────────────
  let showTutorial = $state(false);

  // ── Settings modal ────────────────────────────────────────────────────────
  let settingsOpen = $state(false);

  // ── Git state (fed to SecretsList → GitStorageBar) ────────────────────────
  let gitRemotes = $state<GitRemoteInfo[]>([]);
  let activeGitUrl = $state<string | null>(null);

  async function loadGitState() {
    try {
      const s = await getSettings();
      gitRemotes = s.git_remotes;
      activeGitUrl = s.git_remote?.url ?? null;
      if (!s.tutorial_seen) showTutorial = true;
    } catch {
      // Non-fatal: bar just won't show.
    }
  }

  onMount(loadGitState);

  function handleSettingsClose() {
    settingsOpen = false;
    loadGitState();
  }

  /**
   * Called by SettingsModal when the vault data was replaced (connect / switch /
   * disconnect / remove).  Refreshes the namespace list and resets UI state so the
   * sidebar reflects the new repo's data immediately.
   */
  async function handleVaultReplaced() {
    activeSecretId = '';
    creatingNew = false;
    activeNamespaceId.set('');
    try {
      const ns = await listNamespaces();
      namespaces.set(ns);
      if (ns.length > 0) activeNamespaceId.set(ns[0].id);
    } catch {
      // Non-fatal — sidebar will show empty.
    }
    await loadGitState();
  }

  async function handleGitSwitch(url: string) {
    try {
      // Backend: updates active URL then syncs from the new remote into SQLite.
      await switchGitRemote(url);
      activeGitUrl = url;
      // Reset UI to reflect the new repo's namespace/secret set.
      activeSecretId = '';
      creatingNew = false;
      activeNamespaceId.set('');
      const ns = await listNamespaces();
      namespaces.set(ns);
      if (ns.length > 0) activeNamespaceId.set(ns[0].id);
      await loadGitState();
    } catch (e: unknown) {
      console.error('switch_git_remote failed', e);
    }
  }

  /**
   * Called by SecretsList and SecretDetail after any vault mutation
   * (create, update, delete).  Triggers a git sync when in git mode so that
   * the change is immediately pushed to the remote repository.
   */
  async function afterMutation() {
    if (!activeGitUrl) return;
    try {
      await syncGit();
    } catch (e: unknown) {
      // Non-fatal: the change is already in the local git DB; user can sync manually.
      console.error('git auto-sync failed after mutation', e);
    }
    await loadGitState();
  }

  // ── Panel resize ─────────────────────────────────────────────────────────
  const SIDEBAR_MIN = 48;
  const SIDEBAR_MAX = 200;
  let sidebarWidth = $state(64);

  const LIST_MIN = 160;
  const LIST_MAX = 480;
  let listWidth = $state(260);

  let dragging = $state(false);

  function makeDragger(
    getStart: () => number,
    setWidth: (w: number) => void,
    min: number,
    max: number,
  ) {
    return (e: MouseEvent) => {
      e.preventDefault();
      dragging = true;
      const startX = e.clientX;
      const startW = getStart();

      function onMove(ev: MouseEvent) {
        setWidth(Math.max(min, Math.min(max, startW + (ev.clientX - startX))));
      }
      function onUp() {
        dragging = false;
        window.removeEventListener('mousemove', onMove);
        window.removeEventListener('mouseup', onUp);
      }
      window.addEventListener('mousemove', onMove);
      window.addEventListener('mouseup', onUp);
    };
  }

  const onSidebarHandleMousedown = makeDragger(
    () => sidebarWidth,
    (w) => (sidebarWidth = w),
    SIDEBAR_MIN,
    SIDEBAR_MAX,
  );

  const onListHandleMousedown = makeDragger(
    () => listWidth,
    (w) => (listWidth = w),
    LIST_MIN,
    LIST_MAX,
  );

  // Show namespace labels inside the sidebar when it's wide enough.
  const showLabels = $derived(sidebarWidth >= 90);
</script>

<div
  class="app-shell"
  class:resizing={dragging}
  style="--sidebar-width: {sidebarWidth}px; --list-width: {listWidth}px"
>
  <NamespaceSidebar
    {showLabels}
    onOpenSettings={() => (settingsOpen = true)}
    onAfterMutation={() => void afterMutation()}
  />

  <!-- Drag handle between sidebar and list -->
  <button
    class="resize-handle"
    aria-label="Resize namespace sidebar"
    onmousedown={onSidebarHandleMousedown}
    onkeydown={(e) => {
      if (e.key === 'ArrowLeft') sidebarWidth = Math.max(SIDEBAR_MIN, sidebarWidth - 10);
      if (e.key === 'ArrowRight') sidebarWidth = Math.min(SIDEBAR_MAX, sidebarWidth + 10);
    }}
  ></button>

  <SecretsList
    bind:activeSecretId
    onCreateNew={handleCreateNew}
    onSecretDeleted={handleSecretDeleted}
    {gitRemotes}
    {activeGitUrl}
    onOpenSettings={() => (settingsOpen = true)}
    onGitSwitch={handleGitSwitch}
    onAfterMutation={afterMutation}
  />

  <!-- Drag handle between list and detail -->
  <button
    class="resize-handle"
    aria-label="Resize secrets panel"
    onmousedown={onListHandleMousedown}
    onkeydown={(e) => {
      if (e.key === 'ArrowLeft') listWidth = Math.max(LIST_MIN, listWidth - 20);
      if (e.key === 'ArrowRight') listWidth = Math.min(LIST_MAX, listWidth + 20);
    }}
  ></button>

  <SecretDetail
    secretId={activeSecretId}
    isCreatingNew={creatingNew}
    onSaved={handleSecretSaved}
    onCancelled={handleCancelled}
    onAfterMutation={afterMutation}
  />
</div>

<SettingsModal
  open={settingsOpen}
  onClose={handleSettingsClose}
  onVaultReplaced={() => void handleVaultReplaced()}
/>

<!-- Help button (fixed, bottom-right) -->
<button
  class="help-btn"
  title="Show tutorial"
  aria-label="Show tutorial"
  onclick={() => (showTutorial = true)}
>
  <svg viewBox="0 0 20 20" fill="none" aria-hidden="true">
    <circle cx="10" cy="10" r="8.5" stroke="currentColor" stroke-width="1.5"/>
    <path d="M7.5 7.5a2.5 2.5 0 0 1 4.87.83c0 1.67-2.5 2.5-2.5 2.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
    <circle cx="10" cy="14.5" r="0.75" fill="currentColor"/>
  </svg>
</button>

{#if showTutorial}
  <OnboardingTutorial onClose={() => (showTutorial = false)} />
{/if}

<style>
  .app-shell {
    display: flex;
    height: 100vh;
    width: 100%;
    overflow: hidden;
    background: var(--bg-a);
  }

  .app-shell.resizing { user-select: none; cursor: col-resize; }

  .resize-handle {
    width: 5px;
    flex-shrink: 0;
    background: var(--border);
    cursor: col-resize;
    transition: background 0.15s;
    position: relative;
    z-index: 10;
  }
  .resize-handle:hover,
  .resize-handle:focus-visible {
    background: var(--brand-dark);
    outline: none;
  }

  /* ── Help button ─────────────────────────────────────────────── */
  :global(.help-btn) {
    position: fixed;
    bottom: 14px;
    right: 14px;
    z-index: 500;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: var(--bg-b);
    border: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    cursor: pointer;
    transition: background 0.15s, color 0.15s, box-shadow 0.15s;
  }
  :global(.help-btn:hover) {
    background: var(--brand);
    color: var(--brand-dark);
    box-shadow: 0 2px 8px var(--shadow);
  }
  :global(.help-btn svg) { width: 18px; height: 18px; }
</style>
