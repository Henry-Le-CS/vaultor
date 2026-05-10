<script lang="ts">
  import KeyValueEditor from '../secrets/KeyValueEditor.svelte';
  import FileSecretEditor from '../secrets/FileSecretEditor.svelte';
  import { secrets } from '../../stores/secrets.js';
  import { activeNamespaceId } from '../../stores/namespace.js';
  import { session } from '../../stores/session.js';
  import {
    getKvSecret,
    createKvSecret,
    updateKvSecret,
    createFileSecret,
    getFileSecret,
    updateFileSecret,
    unlockVault,
    commitDraft,
    discardDraft,
    type KvFieldDecrypted,
    type KvFieldInput,
  } from '../../api.js';

  interface Props {
    secretId: string;
    isCreatingNew: boolean;
    onSaved: (id: string) => void;
    onCancelled: () => void;
  }

  let { secretId, isCreatingNew, onSaved, onCancelled }: Props = $props();

  // ── New secret type picker ────────────────────────────────
  let newKind = $state<'kv' | 'file' | null>(null);

  // ── KV view state ─────────────────────────────────────────
  let kvFields = $state<KvFieldDecrypted[]>([]);
  let revealed = $state<Set<string>>(new Set());
  let copying = $state<string | null>(null);

  // ── KV / file edit state ──────────────────────────────────
  let editMode = $state(false);
  let editName = $state('');
  let editFields = $state<KvFieldInput[]>([]);

  // ── New KV state ──────────────────────────────────────────
  let newKvName = $state('');
  let newKvFields = $state<KvFieldInput[]>([{ title: '', value: '', hidden: true }]);

  // ── File view state ───────────────────────────────────────
  let fileFilename = $state('');
  let fileB64 = $state('');
  let fileInfo = $state<{ filename: string; size_bytes: number } | null>(null);
  let fileRevealed = $state(false);

  // ── New file state ────────────────────────────────────────
  let newFileName = $state('');
  let newFileFilename = $state('');
  let newFileB64 = $state('');

  let saving = $state(false);

  // ── Lazy-decrypt state ────────────────────────────────────
  // Credentials are NEVER decrypted automatically.
  // ensureLoaded() is called only on user actions (reveal / copy / download / edit).
  let fieldsLoaded = $state(false);
  let loadError = $state('');
  let loadingAction = $state(false);

  // ── Helpers ───────────────────────────────────────────────
  function b64ToText(b64: string): string {
    if (!b64) return '';
    try {
      const bin = atob(b64);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      return new TextDecoder('utf-8', { fatal: false }).decode(bytes);
    } catch { return ''; }
  }

  // Reset all sensitive state whenever the viewed secret, session, or
  // "creating" mode changes. Never auto-loads — every decrypt is user-initiated.
  $effect(() => {
    // Track these reactive sources so the effect re-runs when they change.
    void secretId; void isCreatingNew; void $session.active;

    kvFields = [];
    revealed = new Set();
    loadError = '';
    editMode = false;
    fileInfo = null;
    fileB64 = '';
    fileFilename = '';
    fieldsLoaded = false;
    fileRevealed = false;
  });

  $effect(() => {
    if (isCreatingNew) {
      newKind = null;
      newKvName = '';
      newKvFields = [{ title: '', value: '', hidden: true }];
      newFileName = '';
      newFileFilename = '';
      newFileB64 = '';
    }
  });

  // ── Lazy decrypt + inline re-auth ─────────────────────────
  // Returns true when decrypted data is ready, false if auth failed or errored.
  async function ensureLoaded(): Promise<boolean> {
    if (fieldsLoaded) return true;
    loadingAction = true;
    loadError = '';
    try {
      if (!$session.active) {
        const status = await unlockVault();
        if (status.active) {
          session.open(status.expires_at_ms);
        } else {
          return false;
        }
      }
      const meta = $secrets.find((s) => s.id === secretId);
      if (meta?.kind === 'kv') {
        kvFields = await getKvSecret(secretId);
      } else if (meta?.kind === 'file') {
        const info = await getFileSecret(secretId);
        fileB64 = info.content_b64;
        fileInfo = { filename: info.filename, size_bytes: info.size_bytes };
        fileFilename = info.filename;
      }
      fieldsLoaded = true;
      return true;
    } catch (err: unknown) {
      loadError = err instanceof Error ? err.message : String(err);
      return false;
    } finally {
      loadingAction = false;
    }
  }

  // ── Create KV ─────────────────────────────────────────────
  async function handleCreateKv() {
    saving = true;
    try {
      const meta = await createKvSecret($activeNamespaceId, newKvName, newKvFields);
      secrets.update((list) => [...list, meta]);
      onSaved(meta.id);
    } catch (err: unknown) {
      alert(err instanceof Error ? err.message : String(err));
    } finally {
      saving = false;
    }
  }

  // ── Create File ───────────────────────────────────────────
  async function handleCreateFile() {
    saving = true;
    const fn = newFileFilename || newFileName || 'content.txt';
    try {
      const meta = await createFileSecret($activeNamespaceId, newFileName, fn, newFileB64);
      secrets.update((list) => [...list, meta]);
      onSaved(meta.id);
    } catch (err: unknown) {
      alert(err instanceof Error ? err.message : String(err));
    } finally {
      saving = false;
    }
  }

  // ── Edit KV ───────────────────────────────────────────────
  async function startEditKv() {
    if (!fieldsLoaded && !await ensureLoaded()) return;
    const meta = $secrets.find((s) => s.id === secretId);
    editName = meta?.name ?? '';
    editFields = kvFields.map((f) => ({ title: f.title, value: f.value, hidden: f.hidden }));
    editMode = true;
  }

  async function handleUpdateKv() {
    saving = true;
    try {
      await updateKvSecret(secretId, editName, editFields);
      secrets.update((list) =>
        list.map((s) => (s.id === secretId ? { ...s, name: editName } : s)),
      );
      editMode = false;
      fieldsLoaded = false;
      kvFields = [];
      await ensureLoaded();
    } catch (err: unknown) {
      alert(err instanceof Error ? err.message : String(err));
    } finally {
      saving = false;
    }
  }

  // ── Edit File ─────────────────────────────────────────────
  async function startEditFile() {
    if (!fieldsLoaded && !await ensureLoaded()) return;
    editName = secretName;
    editMode = true;
  }

  async function handleUpdateFile() {
    saving = true;
    const fn = fileFilename || secretName || 'content.txt';
    try {
      await updateFileSecret(secretId, fn, fileB64);
      editMode = false;
      fieldsLoaded = false;
      fileB64 = '';
      fileInfo = null;
      await ensureLoaded();
    } catch (err: unknown) {
      alert(err instanceof Error ? err.message : String(err));
    } finally {
      saving = false;
    }
  }

  // ── KV field actions ──────────────────────────────────────
  async function toggleReveal(fieldId: string) {
    if (!await ensureLoaded()) return;
    const next = new Set(revealed);
    if (next.has(fieldId)) next.delete(fieldId);
    else next.add(fieldId);
    revealed = next;
  }

  async function copyField(fieldId: string) {
    if (!await ensureLoaded()) return;
    const field = kvFields.find((f) => f.id === fieldId);
    if (!field) return;
    await navigator.clipboard.writeText(field.value);
    copying = fieldId;
    setTimeout(() => (copying = null), 1500);
  }

  // ── File actions ──────────────────────────────────────────
  async function copyFileContent() {
    if (!await ensureLoaded()) return;
    await navigator.clipboard.writeText(b64ToText(fileB64));
    copying = 'file-content';
    setTimeout(() => (copying = null), 1500);
  }

  async function downloadFile() {
    if (!await ensureLoaded()) return;
    if (!fileB64 || !fileInfo) return;
    const bin = atob(fileB64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    const blob = new Blob([bytes]);
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = fileInfo.filename;
    a.click();
    URL.revokeObjectURL(url);
  }

  const secretName = $derived($secrets.find((s) => s.id === secretId)?.name ?? '');
  const secretKind = $derived($secrets.find((s) => s.id === secretId)?.kind ?? 'kv');
  const isDraft = $derived($secrets.find((s) => s.id === secretId)?.is_draft ?? false);

  // ── Draft handlers ────────────────────────────────────────
  async function handleCommitDraft() {
    saving = true;
    try {
      await commitDraft(secretId);
      secrets.update((list) =>
        list.map((s) => (s.id === secretId ? { ...s, is_draft: false } : s)),
      );
    } catch (err: unknown) {
      alert(err instanceof Error ? err.message : String(err));
    } finally {
      saving = false;
    }
  }

  async function handleDiscardDraft() {
    if (!confirm('Discard this draft? The secret will be deleted.')) return;
    try {
      await discardDraft(secretId);
      secrets.update((list) => list.filter((s) => s.id !== secretId));
      onCancelled();
    } catch (err: unknown) {
      alert(err instanceof Error ? err.message : String(err));
    }
  }
</script>

<section class="secret-detail" aria-label="Secret detail">

  {#if isCreatingNew && newKind === null}
    <header class="detail-header"><span class="detail-title">New Secret</span></header>
    <div class="type-picker">
      <p class="picker-label">Choose secret type</p>
      <div class="type-cards">
        <button class="type-card" onclick={() => (newKind = 'kv')}>
          <svg viewBox="0 0 32 32" fill="none" aria-hidden="true"><circle cx="12" cy="20" r="7" stroke="var(--brand-dark)" stroke-width="2"/><path d="M18 14.5l9-5.5" stroke="var(--brand-dark)" stroke-width="2" stroke-linecap="round"/><line x1="25" y1="9" x2="25" y2="13" stroke="var(--brand-dark)" stroke-width="2" stroke-linecap="round"/><line x1="21" y1="11" x2="21" y2="15" stroke="var(--brand-dark)" stroke-width="2" stroke-linecap="round"/></svg>
          <span>Key-Value</span>
          <small>Passwords, tokens, API keys</small>
        </button>
        <button class="type-card" onclick={() => (newKind = 'file')}>
          <svg viewBox="0 0 32 32" fill="none" aria-hidden="true"><path d="M6 4h14l6 6v18H6V4z" stroke="var(--brand-dark)" stroke-width="2" stroke-linejoin="round"/><path d="M20 4v6h6" stroke="var(--brand-dark)" stroke-width="2" stroke-linecap="round"/></svg>
          <span>File</span>
          <small>SSH keys, certs, config files</small>
        </button>
      </div>
      <button class="btn-cancel" onclick={onCancelled}>Cancel</button>
    </div>

  {:else if isCreatingNew && newKind === 'kv'}
    <header class="detail-header"><span class="detail-title">New Key-Value Secret</span></header>
    <KeyValueEditor bind:name={newKvName} bind:fields={newKvFields} {saving} onSave={handleCreateKv} onCancel={onCancelled} />

  {:else if isCreatingNew && newKind === 'file'}
    <header class="detail-header"><span class="detail-title">New File Secret</span></header>
    <FileSecretEditor
      bind:name={newFileName}
      bind:filename={newFileFilename}
      contentB64={newFileB64}
      {saving}
      onSave={handleCreateFile}
      onCancel={onCancelled}
      onFileChosen={(fn, b64) => { newFileFilename = fn; newFileB64 = b64; }}
    />

  {:else if !secretId}
    <div class="empty-state">
      <svg viewBox="0 0 64 64" fill="none" aria-hidden="true">
        <circle cx="24" cy="36" r="12" stroke="var(--border)" stroke-width="2.5"/>
        <line x1="33" y1="30" x2="56" y2="18" stroke="var(--border)" stroke-width="2.5" stroke-linecap="round"/>
        <line x1="50" y1="22" x2="50" y2="28" stroke="var(--border)" stroke-width="2.5" stroke-linecap="round"/>
        <line x1="56" y1="18" x2="56" y2="24" stroke="var(--border)" stroke-width="2.5" stroke-linecap="round"/>
        <circle cx="22" cy="38" r="3" fill="var(--border)"/>
      </svg>
      <p>No secret selected</p>
      <span>Choose a secret from the list, or create a new one.</span>
    </div>

  {:else if editMode && secretKind === 'kv'}
    <header class="detail-header"><span class="detail-title">Edit — {secretName}</span></header>
    <KeyValueEditor bind:name={editName} bind:fields={editFields} {saving} onSave={handleUpdateKv} onCancel={() => (editMode = false)} />

  {:else if editMode && secretKind === 'file'}
    <header class="detail-header"><span class="detail-title">Edit — {secretName}</span></header>
    <FileSecretEditor
      bind:name={editName}
      bind:filename={fileFilename}
      contentB64={fileB64}
      {saving}
      onSave={handleUpdateFile}
      onCancel={() => (editMode = false)}
      onFileChosen={(fn, b64) => { fileFilename = fn; fileB64 = b64; }}
    />

  {:else if secretKind === 'kv'}
    <!-- ── KV view ── -->
    {#if isDraft}
      <div class="draft-banner" role="status">
        <span>Unsaved draft</span>
        <div class="draft-actions">
          <button class="btn-discard" onclick={handleDiscardDraft} disabled={saving}>Discard</button>
          <button class="btn-commit" onclick={handleCommitDraft} disabled={saving}>
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    {/if}
    <header class="detail-header">
      <span class="detail-title">{secretName}</span>
      <button class="btn-edit" onclick={startEditKv} disabled={loadingAction}>
        <svg viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M11 2l3 3-8.5 8.5L2 14l.5-3.5L11 2z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/></svg>
        Edit
      </button>
    </header>

    {#if loadError}
      <div class="error-banner" role="alert">
        {loadError}
        <button class="retry-link" onclick={() => { loadError = ''; ensureLoaded(); }}>Retry</button>
      </div>
    {:else if !fieldsLoaded}
      <!-- Locked placeholder — no data in memory -->
      <div class="cred-locked">
        {#if loadingAction}
          <div class="spin-ring"></div>
          <p class="lock-sub">Decrypting…</p>
        {:else}
          <svg viewBox="0 0 48 48" fill="none" aria-hidden="true">
            <rect x="8" y="22" width="32" height="22" rx="4" stroke="var(--muted)" stroke-width="2"/>
            <path d="M14 22v-6a10 10 0 0 1 20 0v6" stroke="var(--muted)" stroke-width="2" stroke-linecap="round"/>
            <circle cx="24" cy="33" r="3" fill="var(--muted)"/>
          </svg>
          <p class="lock-title">Credentials hidden</p>
          <p class="lock-sub">{$session.active ? 'Click to decrypt and view' : 'Session expired — will re-authenticate'}</p>
          <button class="btn-unlock" onclick={() => ensureLoaded()}>
            {$session.active ? 'View credentials' : 'Authenticate & view'}
          </button>
        {/if}
      </div>
    {:else}
      <!-- Fields loaded -->
      <div class="fields-list">
        {#each kvFields as field (field.id)}
          <div class="field-card">
            <div class="field-header">
              <span class="field-title">{field.title || '(untitled)'}</span>
              <div class="field-actions">
                <button class="icon-btn" onclick={() => toggleReveal(field.id)} title={revealed.has(field.id) ? 'Hide' : 'Show'}>
                  {#if revealed.has(field.id)}
                    <svg viewBox="0 0 20 20" fill="none"><path d="M2 10s3-6 8-6 8 6 8 6-3 6-8 6-8-6-8-6z" stroke="currentColor" stroke-width="1.5"/><circle cx="10" cy="10" r="2.5" stroke="currentColor" stroke-width="1.5"/></svg>
                  {:else}
                    <svg viewBox="0 0 20 20" fill="none"><path d="M2 10s3-6 8-6 8 6 8 6-3 6-8 6-8-6-8-6z" stroke="currentColor" stroke-width="1.5"/><line x1="2" y1="3" x2="18" y2="17" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
                  {/if}
                </button>
                <button class="icon-btn" onclick={() => copyField(field.id)} title="Copy">
                  {#if copying === field.id}
                    <svg viewBox="0 0 20 20" fill="none"><path d="M4 10l5 5 7-8" stroke="var(--ok)" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>
                  {:else}
                    <svg viewBox="0 0 20 20" fill="none"><rect x="7" y="3" width="10" height="12" rx="2" stroke="currentColor" stroke-width="1.4"/><path d="M5 5H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2v-1" stroke="currentColor" stroke-width="1.4"/></svg>
                  {/if}
                </button>
              </div>
            </div>
            <div class="field-value">
              {#if revealed.has(field.id)}
                <code class="value-text">{field.value}</code>
              {:else}
                <span class="value-masked">••••••••••••</span>
              {/if}
            </div>
          </div>
        {/each}
        {#if kvFields.length === 0}<p class="muted-hint">No fields.</p>{/if}
      </div>
    {/if}

  {:else if secretKind === 'file'}
    <!-- ── File view ── -->
    <header class="detail-header">
      <span class="detail-title">{secretName}</span>
      <div class="header-actions">
        <button class="btn-edit" onclick={startEditFile} disabled={loadingAction}>
          <svg viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M11 2l3 3-8.5 8.5L2 14l.5-3.5L11 2z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/></svg>
          Edit
        </button>
        <button class="btn-download" onclick={downloadFile} disabled={loadingAction} title="Download file">
          <svg viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M8 3v7M5 8l3 3 3-3" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/><rect x="2" y="12" width="12" height="2" rx="1" fill="currentColor"/></svg>
          Download
        </button>
      </div>
    </header>

    {#if loadError}
      <div class="error-banner" role="alert">
        {loadError}
        <button class="retry-link" onclick={() => { loadError = ''; ensureLoaded(); }}>Retry</button>
      </div>
    {:else if !fieldsLoaded}
      <div class="cred-locked">
        {#if loadingAction}
          <div class="spin-ring"></div>
          <p class="lock-sub">Decrypting…</p>
        {:else}
          <svg viewBox="0 0 48 48" fill="none" aria-hidden="true">
            <path d="M10 6h18l10 10v26H10V6z" stroke="var(--muted)" stroke-width="2" stroke-linejoin="round"/>
            <path d="M28 6v10h10" stroke="var(--muted)" stroke-width="2" stroke-linecap="round"/>
            <rect x="16" y="24" width="16" height="2" rx="1" fill="var(--muted)"/>
            <rect x="16" y="29" width="12" height="2" rx="1" fill="var(--muted)"/>
            <rect x="16" y="34" width="10" height="2" rx="1" fill="var(--muted)"/>
          </svg>
          <p class="lock-title">File content hidden</p>
          <p class="lock-sub">{$session.active ? 'Click to decrypt and view' : 'Session expired — will re-authenticate'}</p>
          <button class="btn-unlock" onclick={() => ensureLoaded()}>
            {$session.active ? 'View file content' : 'Authenticate & view'}
          </button>
        {/if}
      </div>
    {:else}
      <!-- File content loaded -->
      <div class="file-view">
        <div class="file-view-bar">
          <div class="file-view-meta">
            <svg viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M3 2h8l3 3v9H3V2z" stroke="var(--muted)" stroke-width="1.3" stroke-linejoin="round"/><path d="M11 2v3h3" stroke="var(--muted)" stroke-width="1.3" stroke-linecap="round"/></svg>
            <span class="file-badge">{fileInfo?.filename ?? secretName}</span>
            {#if fileInfo}
              <span class="file-size">{(fileInfo.size_bytes / 1024).toFixed(1)} KB</span>
            {/if}
          </div>
          <div class="file-view-actions">
            <button class="btn-file-action" onclick={() => (fileRevealed = !fileRevealed)}>
              {#if fileRevealed}
                <svg viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M1 8s2.5-5 7-5 7 5 7 5-2.5 5-7 5-7-5-7-5z" stroke="currentColor" stroke-width="1.3"/><circle cx="8" cy="8" r="2" stroke="currentColor" stroke-width="1.3"/></svg>
                Hide
              {:else}
                <svg viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M1 8s2.5-5 7-5 7 5 7 5-2.5 5-7 5-7-5-7-5z" stroke="currentColor" stroke-width="1.3"/><line x1="1" y1="2" x2="15" y2="14" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>
                Show
              {/if}
            </button>
            <button class="btn-file-action" onclick={copyFileContent}>
              {#if copying === 'file-content'}
                <svg viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M3 8l4 4 6-6" stroke="var(--ok)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
                Copied
              {:else}
                <svg viewBox="0 0 16 16" fill="none" aria-hidden="true"><rect x="5" y="2" width="9" height="10" rx="1.5" stroke="currentColor" stroke-width="1.3"/><path d="M3 4H2a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h7a1 1 0 0 0 1-1v-1" stroke="currentColor" stroke-width="1.3"/></svg>
                Copy
              {/if}
            </button>
          </div>
        </div>

        {#if fileRevealed}
          <textarea
            class="file-content-view"
            readonly
            value={b64ToText(fileB64)}
            spellcheck={false}
          ></textarea>
        {:else}
          <div class="file-content-masked">
            <span>Content hidden</span>
            <button class="show-inline-btn" onclick={() => (fileRevealed = true)}>Show file content</button>
          </div>
        {/if}
      </div>
    {/if}
  {/if}

</section>

<style>
  .secret-detail { flex: 1; height: 100%; background: var(--card); overflow: hidden; display: flex; flex-direction: column; }

  .detail-header { display: flex; align-items: center; justify-content: space-between; padding: 14px 20px 10px; border-bottom: 1px solid var(--border); flex-shrink: 0; }
  .detail-title { font-size: 15px; font-weight: 600; color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .header-actions { display: flex; gap: 8px; }

  .btn-edit, .btn-download {
    display: flex; align-items: center; gap: 5px;
    padding: 5px 12px; border: 1px solid var(--border); border-radius: var(--radius-sm);
    font-size: 12px; color: var(--muted); background: var(--bg-b);
    flex-shrink: 0; transition: border-color 0.15s, color 0.15s;
  }
  .btn-edit:hover:not(:disabled), .btn-download:hover:not(:disabled) { border-color: var(--brand-dark); color: var(--brand-dark); }
  .btn-edit:disabled, .btn-download:disabled { opacity: 0.4; cursor: default; }
  .btn-edit svg, .btn-download svg { width: 13px; height: 13px; }

  /* Type picker */
  .type-picker { display: flex; flex-direction: column; align-items: center; gap: 20px; padding: 40px 32px; }
  .picker-label { font-size: 14px; color: var(--muted); }
  .type-cards { display: flex; gap: 16px; }
  .type-card {
    display: flex; flex-direction: column; align-items: center; gap: 10px;
    padding: 24px 28px; border: 2px solid var(--border); border-radius: var(--radius);
    background: var(--bg-a); cursor: pointer; transition: border-color 0.15s, background 0.15s;
    min-width: 140px;
  }
  .type-card:hover { border-color: var(--brand-dark); background: var(--bg-b); }
  .type-card svg { width: 32px; height: 32px; }
  .type-card span { font-size: 14px; font-weight: 600; color: var(--text); }
  .type-card small { font-size: 11px; color: var(--muted); text-align: center; }

  /* Empty state */
  .empty-state { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--muted); padding: 48px 32px; text-align: center; }
  .empty-state svg { width: 64px; height: 64px; opacity: 0.4; }
  .empty-state p { font-size: 16px; font-weight: 500; color: var(--text); opacity: 0.5; }
  .empty-state span { font-size: 13px; color: var(--muted); max-width: 260px; }

  /* Error banner */
  .error-banner {
    margin: 16px; padding: 10px 16px; background: var(--err-bg);
    border: 1px solid var(--err-border); border-radius: var(--radius);
    font-size: 13px; color: var(--err); display: flex; align-items: center; gap: 12px;
  }
  .retry-link { font-size: 12px; color: var(--brand-dark); text-decoration: underline; }

  /* Locked placeholder (both KV and file) */
  .cred-locked {
    flex: 1; display: flex; flex-direction: column; align-items: center;
    justify-content: center; gap: 12px; padding: 48px 32px; text-align: center;
  }
  .cred-locked svg { width: 56px; height: 56px; opacity: 0.45; }
  .lock-title { font-size: 15px; font-weight: 600; color: var(--text); opacity: 0.6; }
  .lock-sub { font-size: 12px; color: var(--muted); }
  .btn-unlock {
    margin-top: 4px; padding: 9px 24px; background: var(--brand-dark); color: white;
    border-radius: var(--radius); font-size: 13px; font-weight: 600; transition: background 0.15s;
  }
  .btn-unlock:hover { background: var(--brand-mid); }

  /* Spinner */
  .spin-ring {
    width: 32px; height: 32px; border: 3px solid var(--border);
    border-top-color: var(--brand-dark); border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* KV fields list */
  .fields-list { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 12px; padding: 16px 20px; }
  .field-card { background: var(--bg-a); border: 1px solid var(--border); border-radius: var(--radius); padding: 12px 14px; display: flex; flex-direction: column; gap: 8px; }
  .field-header { display: flex; align-items: center; justify-content: space-between; }
  .field-title { font-size: 11px; font-weight: 600; color: var(--muted); text-transform: uppercase; letter-spacing: 0.05em; }
  .field-actions { display: flex; gap: 6px; }
  .icon-btn { width: 28px; height: 28px; display: flex; align-items: center; justify-content: center; border-radius: var(--radius-sm); color: var(--muted); transition: background 0.1s, color 0.1s; }
  .icon-btn:hover { background: var(--bg-b); color: var(--brand-dark); }
  .icon-btn svg { width: 16px; height: 16px; }
  .field-value { min-height: 24px; }
  .value-masked { font-size: 16px; color: var(--muted); letter-spacing: 2px; user-select: none; }
  .value-text { font-family: ui-monospace, monospace; font-size: 13px; color: var(--text); word-break: break-all; }
  .muted-hint { font-size: 13px; color: var(--muted); text-align: center; padding: 16px; }

  /* File view */
  .file-view { flex: 1; display: flex; flex-direction: column; min-height: 0; }

  .file-view-bar {
    display: flex; align-items: center; justify-content: space-between;
    padding: 10px 20px; border-bottom: 1px solid var(--border); flex-shrink: 0;
    background: var(--bg-b);
  }
  .file-view-meta { display: flex; align-items: center; gap: 8px; overflow: hidden; }
  .file-view-meta svg { width: 14px; height: 14px; flex-shrink: 0; }
  .file-badge { font-size: 13px; font-weight: 600; color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .file-size { font-size: 11px; color: var(--muted); white-space: nowrap; }

  .file-view-actions { display: flex; gap: 6px; flex-shrink: 0; }
  .btn-file-action {
    display: flex; align-items: center; gap: 5px;
    padding: 5px 12px; border: 1px solid var(--border); border-radius: var(--radius-sm);
    font-size: 12px; color: var(--muted); background: var(--card);
    transition: border-color 0.15s, color 0.15s;
  }
  .btn-file-action:hover { border-color: var(--brand-dark); color: var(--brand-dark); }
  .btn-file-action svg { width: 13px; height: 13px; }

  .file-content-view {
    flex: 1; width: 100%; resize: none;
    background: var(--bg-a); border: none;
    padding: 16px 20px;
    font-family: var(--font-mono); font-size: 12px; color: var(--text);
    line-height: 1.7;
  }
  .file-content-view:focus { outline: none; }

  .file-content-masked {
    flex: 1; display: flex; flex-direction: column;
    align-items: center; justify-content: center; gap: 14px;
    color: var(--muted); font-size: 13px;
  }
  .show-inline-btn {
    padding: 8px 20px; border: 1px solid var(--border);
    border-radius: var(--radius-sm); font-size: 13px; color: var(--muted);
    background: var(--bg-b); transition: border-color 0.15s, color 0.15s;
  }
  .show-inline-btn:hover { border-color: var(--brand-dark); color: var(--brand-dark); }

  /* Draft banner */
  .btn-cancel { padding: 8px 18px; border: 1px solid var(--border); border-radius: var(--radius-sm); font-size: 13px; color: var(--muted); background: var(--card); transition: background 0.15s; }
  .btn-cancel:hover { background: var(--bg-b); }

  .draft-banner { display: flex; align-items: center; justify-content: space-between; padding: 8px 20px; background: var(--err-bg); border-bottom: 1px solid var(--err-border); font-size: 13px; color: var(--err); flex-shrink: 0; }
  .draft-actions { display: flex; gap: 8px; }
  .btn-discard { padding: 4px 12px; border: 1px solid var(--err-border); border-radius: var(--radius-sm); font-size: 12px; color: var(--err); background: transparent; transition: background 0.15s; }
  .btn-discard:hover { background: color-mix(in srgb, var(--err-bg) 80%, var(--err) 20%); }
  .btn-commit { padding: 4px 12px; background: var(--err); color: white; border-radius: var(--radius-sm); font-size: 12px; font-weight: 600; transition: opacity 0.15s; }
  .btn-commit:disabled { opacity: 0.6; cursor: default; }
</style>
