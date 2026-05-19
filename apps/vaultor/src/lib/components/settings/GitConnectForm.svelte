<script lang="ts">
  import {
    testGitConnection,
    connectGitRemote,
    type GitConnectionResult,
  } from '../../api.js';

  interface Props {
    onConnected: () => void;
  }
  let { onConnected }: Props = $props();

  // ── Form state ────────────────────────────────────────────────────────────
  let url = $state('');
  let branches = $state<string[]>([]);
  let selectedBranch = $state('');

  type CheckState = 'idle' | 'checking' | 'success' | 'failure';
  let checkState = $state<CheckState>('idle');
  let checkError = $state('');

  let connecting = $state(false);
  let connectError = $state('');

  let guideOpen = $state(false);

  // ── Test connection ───────────────────────────────────────────────────────
  async function handleTest() {
    if (!url.trim()) return;
    checkState = 'checking';
    checkError = '';
    branches = [];
    selectedBranch = '';
    connectError = '';

    try {
      const result: GitConnectionResult = await testGitConnection(url.trim());
      branches = result.branches;
      selectedBranch = result.default_branch;
      checkState = 'success';
    } catch (e: unknown) {
      checkError = e instanceof Error ? e.message : String(e);
      checkState = 'failure';
    }
  }

  // ── Connect ───────────────────────────────────────────────────────────────
  async function handleConnect() {
    if (!url.trim() || !selectedBranch) return;
    connecting = true;
    connectError = '';
    try {
      await connectGitRemote(url.trim(), selectedBranch);
      onConnected();
    } catch (e: unknown) {
      connectError = e instanceof Error ? e.message : String(e);
    } finally {
      connecting = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && checkState === 'idle') handleTest();
  }
</script>

<div class="connect-form">
  <!-- URL row -->
  <div class="url-row">
    <input
      class="url-input"
      type="text"
      bind:value={url}
      placeholder="git@github.com:user/vault.git"
      aria-label="Remote repository URL"
      onkeydown={handleKeydown}
      disabled={connecting}
    />
    <button
      class="secondary-btn"
      onclick={handleTest}
      disabled={!url.trim() || checkState === 'checking' || connecting}
    >
      {checkState === 'checking' ? 'Checking…' : 'Test connection'}
    </button>
  </div>

  <!-- Status indicator -->
  {#if checkState === 'checking'}
    <p class="status checking">● Checking connection…</p>
  {:else if checkState === 'success'}
    <p class="status success">● Connected</p>
  {:else if checkState === 'failure'}
    <p class="status failure">● {checkError || 'Connection failed'}</p>
  {/if}

  <!-- Branch selector — visible only after success -->
  {#if checkState === 'success'}
    <div class="branch-row">
      <label class="branch-label" for="branch-select">Branch</label>
      <select
        id="branch-select"
        class="branch-select"
        bind:value={selectedBranch}
        disabled={connecting}
      >
        {#each branches as b (b)}
          <option value={b}>{b}</option>
        {/each}
      </select>
      <button
        class="primary-btn"
        onclick={handleConnect}
        disabled={!selectedBranch || connecting}
      >
        {connecting ? 'Switching…' : 'Switch to Git'}
      </button>
    </div>

    {#if connectError}
      <p class="connect-error" role="alert">{connectError}</p>
    {/if}
  {/if}

  <!-- Repository setup guide -->
  <div class="guide">
    <button
      class="guide-toggle"
      onclick={() => (guideOpen = !guideOpen)}
      aria-expanded={guideOpen}
    >
      <svg
        class="guide-chevron"
        class:open={guideOpen}
        viewBox="0 0 12 12"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        aria-hidden="true"
      >
        <path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
      How to set up a git repository
    </button>

    {#if guideOpen}
      <ol class="guide-steps">
        <li>Create a new repository on GitHub (or any git host). Set it to <strong>Private</strong> and initialise it with a README so a branch exists.</li>
        <li>Copy the repository URL:
          <ul>
            <li>SSH: <code>git@github.com:&lt;user&gt;/&lt;repo&gt;.git</code></li>
            <li>HTTPS: <code>https://github.com/&lt;user&gt;/&lt;repo&gt;.git</code></li>
          </ul>
        </li>
        <li>Paste the URL above and click <strong>Test connection</strong>.</li>
        <li>Select a branch and click <strong>Switch to Git</strong>. Vaultor imports any data already in that repository. If it's a fresh repo with no existing vault data, your local secrets are kept — use <strong>Sync Now</strong> in settings to push them to git.</li>
      </ol>
    {/if}
  </div>
</div>

<style>
  .connect-form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  /* ── URL row ─────────────────────────────────────────────────── */
  .url-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .url-input {
    flex: 1;
    min-width: 0;
    height: 32px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-a);
    color: var(--text);
    font-size: 12px;
    font-family: ui-monospace, SFMono-Regular, monospace;
    padding: 0 8px;
  }

  .url-input:focus {
    outline: none;
    border-color: var(--brand-dark);
  }

  .url-input:disabled {
    opacity: 0.6;
  }

  /* ── Status ──────────────────────────────────────────────────── */
  .status {
    font-size: 12px;
    margin: 0;
    font-weight: 500;
  }

  .status.checking { color: #60a5fa; }
  .status.success  { color: #22c55e; }
  .status.failure  { color: var(--err); }

  /* ── Branch row ──────────────────────────────────────────────── */
  .branch-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .branch-label {
    font-size: 13px;
    color: var(--text);
    flex-shrink: 0;
  }

  .branch-select {
    flex: 1;
    min-width: 0;
    height: 32px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-a);
    color: var(--text);
    font-size: 13px;
    padding: 0 6px;
  }

  .branch-select:focus {
    outline: none;
    border-color: var(--brand-dark);
  }

  .connect-error {
    font-size: 12px;
    color: var(--err);
    margin: 0;
  }

  /* ── Buttons ─────────────────────────────────────────────────── */
  .primary-btn {
    height: 32px;
    padding: 0 14px;
    background: var(--brand-dark);
    color: white;
    border-radius: var(--radius-sm);
    font-size: 13px;
    font-weight: 500;
    transition: background 0.1s;
    flex-shrink: 0;
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
    white-space: nowrap;
    flex-shrink: 0;
  }

  .secondary-btn:hover:not(:disabled) { background: var(--bg-b); }
  .secondary-btn:disabled { opacity: 0.5; cursor: default; }

  /* ── Guide ───────────────────────────────────────────────────── */
  .guide {
    border-top: 1px solid var(--border);
    padding-top: 8px;
    margin-top: 2px;
  }

  .guide-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
    transition: color 0.1s;
    padding: 0;
  }

  .guide-toggle:hover { color: var(--text); }

  .guide-chevron {
    width: 12px;
    height: 12px;
    transition: transform 0.15s;
    flex-shrink: 0;
  }

  .guide-chevron.open { transform: rotate(180deg); }

  .guide-steps {
    margin: 10px 0 0 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--text);
    line-height: 1.5;
  }

  .guide-steps ul {
    margin: 4px 0 0 0;
    padding-left: 16px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .guide-steps code {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 11px;
    background: var(--bg-b);
    padding: 1px 4px;
    border-radius: 3px;
    color: var(--muted);
  }
</style>
