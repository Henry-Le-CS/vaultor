<script lang="ts">
  import {
    getSettings,
    setSessionExpiry,
    pickFolder,
    moveStorage,
    type SessionExpiry,
  } from '../../api.js';

  interface Props {
    open: boolean;
    onClose: () => void;
  }
  let { open, onClose }: Props = $props();

  // ── Settings state ───────────────────────────────────────────
  let sessionExpiry = $state<SessionExpiry>('minutes_2');
  let dbPath = $state('');
  let loading = $state(false);
  let error = $state('');

  // ── Storage move state ───────────────────────────────────────
  let newDir = $state('');
  let moveError = $state('');
  let movePending = $state(false);
  let moveSuccess = $state(false);
  let showMoveRow = $state(false);
  let confirmOverwrite = $state(false);

  // ── Load on open ─────────────────────────────────────────────
  $effect(() => {
    if (open) {
      loadSettings();
      moveSuccess = false;
      moveError = '';
      showMoveRow = false;
      newDir = '';
      confirmOverwrite = false;
    }
  });

  async function loadSettings() {
    loading = true;
    error = '';
    try {
      const s = await getSettings();
      sessionExpiry = s.session_expiry;
      dbPath = s.db_path;
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  // ── Session expiry ────────────────────────────────────────────
  async function handleExpiryChange(val: SessionExpiry) {
    sessionExpiry = val;
    try {
      await setSessionExpiry(val);
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  // ── Storage move ──────────────────────────────────────────────
  async function handleBrowse() {
    const picked = await pickFolder();
    if (picked) newDir = picked;
  }

  async function handleMove(force = false) {
    if (!newDir.trim()) return;
    movePending = true;
    moveError = '';
    confirmOverwrite = false;
    try {
      const newPath = await moveStorage(newDir.trim(), force);
      dbPath = newPath;
      moveSuccess = true;
      showMoveRow = false;
      newDir = '';
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg === 'destination_exists') {
        confirmOverwrite = true;
        moveError = 'That folder already has a vaultor.db file.';
      } else {
        moveError = msg || 'Move failed. The original file is untouched.';
      }
    } finally {
      movePending = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }
</script>

{#if open}
  <!-- Backdrop -->
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={onClose}
    onkeydown={handleKeydown}
  ></div>

  <!-- Modal -->
  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    aria-label="Settings"
    tabindex="-1"
    onkeydown={handleKeydown}
  >
    <header class="modal-header">
      <h2>Settings</h2>
      <button class="close-btn" onclick={onClose} aria-label="Close settings">
        <svg viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
          <line x1="5" y1="5" x2="15" y2="15" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
          <line x1="15" y1="5" x2="5" y2="15" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
        </svg>
      </button>
    </header>

    {#if loading}
      <div class="loading">Loading…</div>
    {:else}

      <!-- ── Storage section ─────────────────────────────────── -->
      <section class="settings-section">
        <h3 class="section-title">Storage</h3>

        <div class="field-row">
          <span class="field-label">Vault file</span>
          <div class="path-display">
            <span class="path-text" title={dbPath}>{dbPath}</span>
            <button
              class="copy-btn"
              onclick={() => navigator.clipboard.writeText(dbPath)}
              aria-label="Copy path"
              title="Copy path"
            >
              <svg viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
                <rect x="7" y="5" width="8" height="11" rx="1.5" stroke="currentColor" stroke-width="1.5"/>
                <path d="M5 14.5V5.5A1.5 1.5 0 0 1 6.5 4H12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </button>
          </div>
        </div>

        {#if !showMoveRow}
          <button class="link-btn" onclick={() => (showMoveRow = true)}>
            Change Location…
          </button>
        {:else}
          <div class="move-row">
            <input
              class="path-input"
              bind:value={newDir}
              placeholder="New folder path…"
              aria-label="New vault folder path"
            />
            <button class="secondary-btn" onclick={handleBrowse} disabled={movePending}>
              Browse
            </button>
            <button
              class="primary-btn"
              onclick={() => handleMove(false)}
              disabled={movePending || !newDir.trim()}
            >
              {movePending ? 'Moving…' : 'Move'}
            </button>
            <button class="ghost-btn" onclick={() => { showMoveRow = false; moveError = ''; confirmOverwrite = false; }} disabled={movePending}>
              Cancel
            </button>
          </div>

          {#if moveError}
            <p class="move-error" role="alert">{moveError}</p>
            {#if confirmOverwrite}
              <div class="confirm-row">
                <span>Overwrite the existing file?</span>
                <button class="danger-btn" onclick={() => handleMove(true)} disabled={movePending}>
                  Overwrite
                </button>
                <button class="ghost-btn" onclick={() => { confirmOverwrite = false; moveError = ''; }}>
                  Cancel
                </button>
              </div>
            {/if}
          {/if}
        {/if}

        {#if moveSuccess}
          <p class="success-msg" role="status">
            Vault moved. Please restart Vaultor for the change to take effect.
          </p>
        {/if}
      </section>

      <!-- ── Session section ────────────────────────────────── -->
      <section class="settings-section">
        <h3 class="section-title">Session</h3>

        <div class="field-row">
          <span class="field-label">Lock after</span>
          <div class="expiry-group" role="group" aria-label="Session expiry">
            {#each [
              { value: 'minutes_2',  label: '2 min' },
              { value: 'minutes_5',  label: '5 min' },
              { value: 'minutes_10', label: '10 min' },
              { value: 'until_quit', label: 'Until quit — less secure' },
            ] as opt (opt.value)}
              <button
                class="expiry-btn"
                class:active={sessionExpiry === opt.value}
                onclick={() => handleExpiryChange(opt.value as SessionExpiry)}
                aria-pressed={sessionExpiry === opt.value}
              >
                {opt.label}
              </button>
            {/each}
          </div>
        </div>

        <p class="hint">
          Changes take effect on the next unlock.
          The current session keeps its original timeout.
        </p>
      </section>

      {#if error}
        <p class="error-msg" role="alert">{error}</p>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    z-index: 900;
  }

  .modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 910;
    width: min(540px, 90vw);
    max-height: 80vh;
    overflow-y: auto;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 8px 40px var(--shadow);
    display: flex;
    flex-direction: column;
  }

  /* ── Header ─────────────────────────────────────────────── */
  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 20px 14px;
    border-bottom: 1px solid var(--border);
  }

  .modal-header h2 {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
    margin: 0;
  }

  .close-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    color: var(--muted);
    transition: background 0.1s, color 0.1s;
  }

  .close-btn:hover { background: var(--bg-b); color: var(--text); }
  .close-btn svg { width: 16px; height: 16px; }

  /* ── Sections ────────────────────────────────────────────── */
  .settings-section {
    padding: 18px 20px;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .settings-section:last-child { border-bottom: none; }

  .section-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--muted);
    margin: 0;
  }

  /* ── Field rows ──────────────────────────────────────────── */
  .field-row {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    min-width: 0;
  }

  .field-label {
    font-size: 13px;
    color: var(--text);
    min-width: 72px;
    padding-top: 6px;
    flex-shrink: 0;
  }

  /* ── Path display ────────────────────────────────────────── */
  .path-display {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-b);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 5px 8px;
  }

  .path-text {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    font-family: ui-monospace, SFMono-Regular, monospace;
    color: var(--muted);
    white-space: nowrap;
    overflow-x: auto;
    /* hide scrollbar chrome — user can still scroll by dragging */
    scrollbar-width: none;
  }

  .path-text::-webkit-scrollbar {
    display: none;
  }

  .copy-btn {
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    color: var(--muted);
    transition: background 0.1s, color 0.1s;
  }

  .copy-btn:hover { background: var(--brand); color: var(--text); }
  .copy-btn svg { width: 14px; height: 14px; }

  /* ── Move row ────────────────────────────────────────────── */
  .move-row {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }

  .path-input {
    flex: 1;
    min-width: 140px;
    height: 32px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-a);
    color: var(--text);
    font-size: 12px;
    padding: 0 8px;
  }

  .path-input:focus {
    outline: none;
    border-color: var(--brand-dark);
  }

  /* ── Buttons ─────────────────────────────────────────────── */
  .link-btn {
    font-size: 13px;
    color: var(--brand-dark);
    text-decoration: underline;
    text-align: left;
    padding: 0;
  }

  .link-btn:hover { color: var(--brand-mid); }

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

  .secondary-btn {
    height: 32px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--card);
    color: var(--text);
    font-size: 13px;
    transition: background 0.1s;
  }

  .secondary-btn:hover:not(:disabled) { background: var(--bg-b); }
  .secondary-btn:disabled { opacity: 0.5; cursor: default; }

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
    height: 30px;
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

  /* ── Session expiry ──────────────────────────────────────── */
  .expiry-group {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .expiry-btn {
    height: 30px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--card);
    color: var(--muted);
    font-size: 12px;
    font-weight: 500;
    transition: background 0.1s, border-color 0.1s, color 0.1s;
  }

  .expiry-btn:hover { background: var(--bg-b); color: var(--text); }

  .expiry-btn.active {
    background: var(--brand-dark);
    border-color: var(--brand-dark);
    color: white;
  }

  /* ── Hint / messages ─────────────────────────────────────── */
  .hint {
    font-size: 11px;
    color: var(--muted);
    margin: 0;
    line-height: 1.5;
  }

  .move-error {
    font-size: 12px;
    color: var(--err);
    margin: 0;
  }

  .confirm-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text);
  }

  .success-msg {
    font-size: 12px;
    color: var(--brand-dark);
    margin: 0;
  }

  .error-msg {
    margin: 8px 20px;
    font-size: 12px;
    color: var(--err);
    padding: 6px 10px;
    background: var(--err-bg);
    border: 1px solid var(--err-border);
    border-radius: var(--radius-sm);
  }

  .loading {
    padding: 24px 20px;
    font-size: 13px;
    color: var(--muted);
    text-align: center;
  }
</style>
