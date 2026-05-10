<script lang="ts">
  import { onMount } from 'svelte';

  interface Props {
    name: string;
    filename: string;
    contentB64: string;
    saving: boolean;
    onSave: () => void;
    onCancel: () => void;
    /** Called whenever the filename or content changes. */
    onFileChosen: (filename: string, b64: string) => void;
  }

  let {
    name = $bindable(''),
    filename = $bindable(''),
    contentB64,
    saving,
    onSave,
    onCancel,
    onFileChosen,
  }: Props = $props();

  // ── Text content shown in the textarea ───────────────────
  // Managed locally. Populated from the incoming base64 on mount (one-time),
  // then updated directly when a file is dropped or picked.
  // Typing in the textarea never re-reads the prop, avoiding a sync loop.
  let textContent = $state('');

  onMount(() => {
    textContent = b64ToText(contentB64);
  });

  let dragging = $state(false);
  let fileInput: HTMLInputElement | null = null;

  // ── Helpers ──────────────────────────────────────────────
  function b64ToText(b64: string): string {
    if (!b64) return '';
    try {
      const binary = atob(b64);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
      return new TextDecoder('utf-8', { fatal: false }).decode(bytes);
    } catch {
      return '';
    }
  }

  function textToB64(text: string): string {
    const bytes = new TextEncoder().encode(text);
    let binary = '';
    for (const b of bytes) binary += String.fromCharCode(b);
    return btoa(binary);
  }

  function bytesToB64(bytes: Uint8Array): string {
    let binary = '';
    for (const b of bytes) binary += String.fromCharCode(b);
    return btoa(binary);
  }

  // ── Textarea change ──────────────────────────────────────
  function onTextChange(e: Event) {
    const val = (e.target as HTMLTextAreaElement).value;
    textContent = val;
    onFileChosen(filename, textToB64(val));
  }

  // ── File reading ─────────────────────────────────────────
  async function readFile(file: File): Promise<void> {
    if (file.size > 1_048_576) {
      alert('File exceeds 1 MiB limit.');
      return;
    }
    const buffer = await file.arrayBuffer();
    const bytes = new Uint8Array(buffer);
    const b64 = bytesToB64(bytes);
    // Decode for textarea display; non-UTF-8 bytes become replacement chars.
    const decoded = new TextDecoder('utf-8', { fatal: false }).decode(bytes);
    textContent = decoded;
    filename = file.name;
    onFileChosen(file.name, b64);
  }

  async function maybeReadFile(file: File): Promise<void> {
    if (textContent.trim() !== '') {
      if (!confirm('Replace current content with the dropped file?')) return;
    }
    await readFile(file);
  }

  // ── Drag & drop ──────────────────────────────────────────
  function onDragover(e: DragEvent) {
    e.preventDefault();
    dragging = true;
  }

  function onDragleave() {
    dragging = false;
  }

  async function onDrop(e: DragEvent) {
    e.preventDefault();
    dragging = false;
    const file = e.dataTransfer?.files[0];
    if (file) await maybeReadFile(file);
  }

  // ── File picker ──────────────────────────────────────────
  async function onInputChange(e: Event) {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (!file) return;
    if (textContent.trim() !== '') {
      if (!confirm('Replace current content with the chosen file?')) return;
    }
    await readFile(file);
    // Reset input so the same file can be re-selected if needed.
    (e.target as HTMLInputElement).value = '';
  }
</script>

<div
  class="file-editor"
  class:dragging
  ondragover={onDragover}
  ondragleave={onDragleave}
  ondrop={onDrop}
  role="region"
  aria-label="File secret editor"
>
  <!-- ── Name ── -->
  <div class="field-row">
    <label class="field-label" for="fs-name">Secret name</label>
    <input
      id="fs-name"
      class="text-input"
      bind:value={name}
      placeholder="e.g. SSH private key"
      disabled={saving}
    />
  </div>

  <!-- ── File picker ── -->
  <div class="field-row">
    <span class="field-label">File</span>
    <div class="file-picker-row">
      <button
        class="btn-browse"
        onclick={() => fileInput?.click()}
        disabled={saving}
        title="Pick a file"
      >
        Browse…
      </button>
      {#if filename}
        <span class="selected-filename">{filename}</span>
      {/if}
    </div>
  </div>

  <!-- ── Content textarea ── -->
  <div class="field-row field-row--grow">
    <label class="field-label" for="fs-content">
      Content
      {#if dragging}<span class="drop-hint">— drop file to import</span>{/if}
    </label>
    <textarea
      id="fs-content"
      class="content-textarea"
      class:drag-over={dragging}
      value={textContent}
      oninput={onTextChange}
      placeholder="Paste or type content here, or drag & drop / browse a file above…"
      disabled={saving}
      spellcheck={false}
    ></textarea>
  </div>

  <!-- hidden file input -->
  <input
    bind:this={fileInput}
    type="file"
    class="hidden-input"
    onchange={onInputChange}
    tabindex="-1"
    aria-hidden="true"
  />

  <!-- ── Actions ── -->
  <div class="actions">
    <button class="btn-cancel" onclick={onCancel} disabled={saving}>Cancel</button>
    <button
      class="btn-save"
      onclick={onSave}
      disabled={saving || !name.trim() || !filename.trim()}
    >
      {saving ? 'Saving…' : 'Save'}
    </button>
  </div>
</div>

<style>
  .file-editor {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 20px 24px;
    height: 100%;
    overflow-y: auto;
    transition: background 0.15s;
  }
  .file-editor.dragging { background: var(--bg-b); }

  .field-row { display: flex; flex-direction: column; gap: 6px; }
  .field-row--grow { flex: 1; min-height: 0; }
  .field-label {
    font-size: 11px; font-weight: 600; color: var(--muted);
    text-transform: uppercase; letter-spacing: 0.05em;
  }
  .drop-hint { font-weight: 400; color: var(--brand-dark); text-transform: none; letter-spacing: 0; }

  .text-input {
    background: var(--bg-b); border: 1px solid var(--border);
    border-radius: var(--radius-sm); padding: 8px 10px;
    font-size: 13px; color: var(--text); transition: border-color 0.15s;
  }
  .text-input:focus { border-color: var(--brand-dark); outline: none; }

  .file-picker-row { display: flex; align-items: center; gap: 10px; }
  .selected-filename {
    font-size: 12px; color: var(--muted); font-family: var(--font-mono);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }

  .btn-browse {
    padding: 8px 14px; border: 1px solid var(--border);
    border-radius: var(--radius-sm); font-size: 13px; color: var(--muted);
    background: var(--bg-b); white-space: nowrap; transition: border-color 0.15s, color 0.15s;
    flex-shrink: 0;
  }
  .btn-browse:hover:not(:disabled) { border-color: var(--brand-dark); color: var(--brand-dark); }
  .btn-browse:disabled { opacity: 0.5; cursor: default; }

  .content-textarea {
    flex: 1; width: 100%; min-height: 180px; resize: none;
    background: var(--bg-b); border: 1px solid var(--border);
    border-radius: var(--radius-sm); padding: 10px 12px;
    font-family: var(--font-mono); font-size: 12px; color: var(--text);
    line-height: 1.6; transition: border-color 0.15s, background 0.15s;
  }
  .content-textarea:focus { border-color: var(--brand-dark); outline: none; }
  .content-textarea.drag-over { border-color: var(--brand-dark); border-style: dashed; background: var(--card); }
  .content-textarea:disabled { opacity: 0.7; cursor: default; }

  .hidden-input { display: none; }

  .actions { display: flex; gap: 10px; justify-content: flex-end; margin-top: auto; flex-shrink: 0; }

  .btn-cancel {
    padding: 8px 18px; border: 1px solid var(--border);
    border-radius: var(--radius-sm); font-size: 13px; color: var(--muted);
    background: var(--card); transition: background 0.15s;
  }
  .btn-cancel:hover { background: var(--bg-b); }

  .btn-save {
    padding: 8px 18px; background: var(--brand-dark); color: white;
    border-radius: var(--radius-sm); font-size: 13px; font-weight: 600;
    transition: background 0.15s;
  }
  .btn-save:hover:not(:disabled) { background: var(--brand-mid); }
  .btn-save:disabled { opacity: 0.5; cursor: default; }
</style>
