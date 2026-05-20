<script lang="ts">
  import { generatePassword } from '../../api.js';

  interface Props {
    onSelect: (password: string) => void;
    onClose: () => void;
  }

  let { onSelect, onClose }: Props = $props();

  let length = $state(16);
  let useUppercase = $state(true);
  let useLowercase = $state(true);
  let useDigits = $state(true);
  let useSymbols = $state(true);

  let password = $state('');
  let generating = $state(false);
  let error = $state('');

  const charsetCount = $derived(
    [useUppercase, useLowercase, useDigits, useSymbols].filter(Boolean).length,
  );

  const strength = $derived.by(() => {
    if (charsetCount === 0) return { label: '', color: '', percent: 0 };
    if (length < 8 || charsetCount <= 1) return { label: 'Weak', color: 'var(--err)', percent: 25 };
    if (length < 12 || charsetCount <= 2) return { label: 'Medium', color: '#e0952d', percent: 50 };
    if (length < 20 || charsetCount <= 3) return { label: 'Strong', color: '#3daa4a', percent: 75 };
    return { label: 'Very Strong', color: '#22863a', percent: 100 };
  });

  async function generate() {
    if (charsetCount === 0) return;
    generating = true;
    error = '';
    try {
      password = await generatePassword({
        length,
        useUppercase,
        useLowercase,
        useDigits,
        useSymbols,
      });
    } catch (e) {
      error = String(e);
    } finally {
      generating = false;
    }
  }

  function usePassword() {
    if (password) {
      onSelect(password);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }

  function handleBackdropClick(e: MouseEvent) {
    if ((e.target as HTMLElement).classList.contains('pw-gen-backdrop')) {
      onClose();
    }
  }

  // Generate on mount.
  $effect(() => {
    generate();
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="pw-gen-backdrop" onclick={handleBackdropClick}>
  <div class="pw-gen-popup">
    <div class="pw-gen-header">
      <span class="pw-gen-title">Password Generator</span>
      <button class="pw-gen-close" onclick={onClose} aria-label="Close">
        <svg viewBox="0 0 16 16" fill="none"><line x1="3" y1="3" x2="13" y2="13" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><line x1="13" y1="3" x2="3" y2="13" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
      </button>
    </div>

    {#if password}
      <div class="pw-preview">
        <code class="pw-value">{password}</code>
        <button class="pw-regen" onclick={generate} disabled={generating || charsetCount === 0} title="Regenerate">
          <svg viewBox="0 0 16 16" fill="none"><path d="M13.5 8a5.5 5.5 0 1 1-1.3-3.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/><polyline points="12 1 13.5 4.5 10 4.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>
        </button>
      </div>
    {/if}

    {#if error}
      <div class="pw-error">{error}</div>
    {/if}

    {#if charsetCount > 0}
      <div class="pw-strength">
        <div class="pw-strength-bar">
          <div class="pw-strength-fill" style="width:{strength.percent}%;background:{strength.color}"></div>
        </div>
        <span class="pw-strength-label" style="color:{strength.color}">{strength.label}</span>
      </div>
    {/if}

    <div class="pw-control">
      <label class="pw-label" for="pw-length">Length: {length}</label>
      <input id="pw-length" type="range" min="4" max="128" bind:value={length} oninput={() => generate()} />
    </div>

    <div class="pw-charsets">
      <label class="pw-checkbox">
        <input type="checkbox" bind:checked={useUppercase} onchange={() => generate()} />
        A-Z
      </label>
      <label class="pw-checkbox">
        <input type="checkbox" bind:checked={useLowercase} onchange={() => generate()} />
        a-z
      </label>
      <label class="pw-checkbox">
        <input type="checkbox" bind:checked={useDigits} onchange={() => generate()} />
        0-9
      </label>
      <label class="pw-checkbox">
        <input type="checkbox" bind:checked={useSymbols} onchange={() => generate()} />
        !@#$
      </label>
    </div>

    <div class="pw-actions">
      <button class="btn-cancel" onclick={onClose}>Cancel</button>
      <button class="btn-save" onclick={usePassword} disabled={!password || generating}>Use Password</button>
    </div>
  </div>
</div>

<style>
  .pw-gen-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.3);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .pw-gen-popup {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: var(--radius-md, 10px);
    padding: 20px;
    width: 340px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
  }

  .pw-gen-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .pw-gen-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
  }

  .pw-gen-close {
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: color 0.1s;
  }
  .pw-gen-close:hover { color: var(--text); }
  .pw-gen-close svg { width: 14px; height: 14px; }

  .pw-preview {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg-b);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 10px 12px;
  }

  .pw-value {
    flex: 1;
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
    font-size: 13px;
    color: var(--text);
    word-break: break-all;
    user-select: all;
  }

  .pw-regen {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
    flex-shrink: 0;
    transition: color 0.1s, background 0.1s;
  }
  .pw-regen:hover { color: var(--brand-dark); background: var(--bg-b); }
  .pw-regen:disabled { opacity: 0.4; cursor: default; }
  .pw-regen svg { width: 14px; height: 14px; }

  .pw-error {
    color: var(--err);
    font-size: 12px;
  }

  .pw-strength {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .pw-strength-bar {
    flex: 1;
    height: 4px;
    background: var(--border);
    border-radius: 2px;
    overflow: hidden;
  }

  .pw-strength-fill {
    height: 100%;
    border-radius: 2px;
    transition: width 0.2s, background 0.2s;
  }

  .pw-strength-label {
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
  }

  .pw-control {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .pw-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }

  .pw-control input[type="range"] {
    width: 100%;
    accent-color: var(--brand-dark);
  }

  .pw-charsets {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }

  .pw-checkbox {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
  }

  .pw-checkbox input[type="checkbox"] {
    accent-color: var(--brand-dark);
  }

  .pw-actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 2px;
  }

  .btn-cancel {
    padding: 8px 18px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-size: 13px;
    color: var(--muted);
    background: var(--card);
    cursor: pointer;
    transition: background 0.15s;
  }
  .btn-cancel:hover { background: var(--bg-b); }

  .btn-save {
    padding: 8px 18px;
    background: var(--brand-dark);
    color: white;
    border: none;
    border-radius: var(--radius-sm);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }
  .btn-save:hover:not(:disabled) { background: var(--brand-mid); }
  .btn-save:disabled { opacity: 0.5; cursor: default; }
</style>
