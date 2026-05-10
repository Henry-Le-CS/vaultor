<script lang="ts">
  import NamespaceSidebar from './NamespaceSidebar.svelte';
  import SecretsList from './SecretsList.svelte';
  import SecretDetail from './SecretDetail.svelte';

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

  // ── Panel resize ─────────────────────────────────────────────
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
  <NamespaceSidebar {showLabels} />

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
  />
</div>

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
</style>
