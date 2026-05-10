<script lang="ts">
  import type { KvFieldInput } from '../../api.js';

  interface Props {
    name: string;
    fields: KvFieldInput[];
    saving: boolean;
    onSave: () => void;
    onCancel: () => void;
  }

  let { name = $bindable(''), fields = $bindable([]), saving, onSave, onCancel }: Props = $props();

  function addField() {
    fields = [...fields, { title: '', value: '', hidden: true }];
  }

  function removeField(i: number) {
    fields = fields.filter((_, idx) => idx !== i);
  }
</script>

<div class="kv-editor">
  <div class="field-row">
    <label class="field-label" for="secret-name">Name</label>
    <input
      id="secret-name"
      class="text-input"
      bind:value={name}
      placeholder="e.g. AWS API Key"
      disabled={saving}
    />
  </div>

  <div class="fields-section">
    <span class="fields-label">Fields</span>

    {#each fields as field, i}
      <div class="kv-row">
        <input
          class="text-input title-input"
          bind:value={field.title}
          placeholder="Title"
          disabled={saving}
        />
        <input
          class="text-input value-input"
          type={field.hidden ? 'password' : 'text'}
          bind:value={field.value}
          placeholder="Value"
          disabled={saving}
        />
        <label class="hidden-toggle" title="Mask this field">
          <input type="checkbox" bind:checked={field.hidden} disabled={saving} />
          <span class="eye-icon" aria-hidden="true">
            {#if field.hidden}
              <!-- Eye closed -->
              <svg viewBox="0 0 20 20" fill="none"><path d="M2 10s3-6 8-6 8 6 8 6-3 6-8 6-8-6-8-6z" stroke="currentColor" stroke-width="1.5"/><line x1="2" y1="3" x2="18" y2="17" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
            {:else}
              <!-- Eye open -->
              <svg viewBox="0 0 20 20" fill="none"><path d="M2 10s3-6 8-6 8 6 8 6-3 6-8 6-8-6-8-6z" stroke="currentColor" stroke-width="1.5"/><circle cx="10" cy="10" r="2.5" stroke="currentColor" stroke-width="1.5"/></svg>
            {/if}
          </span>
        </label>
        <button class="remove-btn" onclick={() => removeField(i)} disabled={saving} aria-label="Remove field">
          <svg viewBox="0 0 16 16" fill="none"><line x1="3" y1="3" x2="13" y2="13" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><line x1="13" y1="3" x2="3" y2="13" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
        </button>
      </div>
    {/each}

    <button class="add-field-btn" onclick={addField} disabled={saving}>
      <svg viewBox="0 0 16 16" fill="none" aria-hidden="true"><line x1="8" y1="3" x2="8" y2="13" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><line x1="3" y1="8" x2="13" y2="8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
      Add field
    </button>
  </div>

  <div class="actions">
    <button class="btn-cancel" onclick={onCancel} disabled={saving}>Cancel</button>
    <button class="btn-save" onclick={onSave} disabled={saving || !name.trim()}>
      {saving ? 'Saving…' : 'Save'}
    </button>
  </div>
</div>

<style>
  .kv-editor {
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 24px;
    height: 100%;
    overflow-y: auto;
  }

  .field-row { display: flex; flex-direction: column; gap: 6px; }
  .field-label { font-size: 12px; font-weight: 600; color: var(--muted); text-transform: uppercase; letter-spacing: 0.05em; }

  .fields-section { display: flex; flex-direction: column; gap: 8px; }
  .fields-label { font-size: 12px; font-weight: 600; color: var(--muted); text-transform: uppercase; letter-spacing: 0.05em; }

  .kv-row { display: flex; gap: 8px; align-items: center; }

  .text-input {
    background: var(--bg-b);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 8px 10px;
    font-size: 13px;
    color: var(--text);
    transition: border-color 0.15s;
    width: 100%;
  }
  .text-input:focus { border-color: var(--brand-dark); outline: none; }

  .title-input { flex: 1; min-width: 0; }
  .value-input { flex: 2; min-width: 0; }

  .hidden-toggle {
    display: flex;
    align-items: center;
    cursor: pointer;
    flex-shrink: 0;
  }
  .hidden-toggle input { display: none; }
  .eye-icon svg { width: 18px; height: 18px; color: var(--muted); }
  .hidden-toggle:hover .eye-icon svg { color: var(--brand-dark); }

  .remove-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    border-radius: var(--radius-sm);
    flex-shrink: 0;
    transition: color 0.1s, background 0.1s;
  }
  .remove-btn:hover { color: var(--err); background: var(--err-bg); }
  .remove-btn svg { width: 14px; height: 14px; }

  .add-field-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border: 1.5px dashed var(--border);
    border-radius: var(--radius-sm);
    color: var(--muted);
    font-size: 13px;
    transition: border-color 0.15s, color 0.15s;
    align-self: flex-start;
  }
  .add-field-btn:hover { border-color: var(--brand-dark); color: var(--brand-dark); }
  .add-field-btn svg { width: 14px; height: 14px; }

  .actions { display: flex; gap: 10px; justify-content: flex-end; margin-top: auto; }

  .btn-cancel {
    padding: 8px 18px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-size: 13px;
    color: var(--muted);
    background: var(--card);
    transition: background 0.15s;
  }
  .btn-cancel:hover { background: var(--bg-b); }

  .btn-save {
    padding: 8px 18px;
    background: var(--brand-dark);
    color: white;
    border-radius: var(--radius-sm);
    font-size: 13px;
    font-weight: 600;
    transition: background 0.15s;
  }
  .btn-save:hover:not(:disabled) { background: var(--brand-mid); }
  .btn-save:disabled { opacity: 0.5; cursor: default; }
</style>
