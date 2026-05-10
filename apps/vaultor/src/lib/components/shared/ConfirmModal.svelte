<script lang="ts">
  interface Props {
    title: string;
    message: string;
    confirmLabel?: string;
    /**
     * When set, the confirm button is disabled until the user types this exact
     * phrase into the input.  Use for destructive, hard-to-reverse actions.
     */
    requiredPhrase?: string;
    onConfirm: () => void;
    onCancel: () => void;
  }

  let {
    title,
    message,
    confirmLabel = 'Delete',
    requiredPhrase,
    onConfirm,
    onCancel,
  }: Props = $props();

  let typed = $state('');
  let inputEl = $state<HTMLInputElement | null>(null);

  const canConfirm = $derived(!requiredPhrase || typed === requiredPhrase);

  // Auto-focus: input when typed phrase required, confirm button otherwise.
  $effect(() => {
    if (requiredPhrase) {
      setTimeout(() => inputEl?.focus(), 0);
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onCancel();
    if (e.key === 'Enter' && canConfirm) onConfirm();
  }
</script>

<!-- Backdrop -->
<div
  class="backdrop"
  role="presentation"
  onclick={onCancel}
></div>

<!-- Dialog -->
<div
  class="dialog"
  role="alertdialog"
  aria-modal="true"
  aria-labelledby="confirm-title"
  aria-describedby="confirm-msg"
  tabindex="-1"
  onkeydown={handleKeydown}
>
  <h3 id="confirm-title" class="dialog-title">{title}</h3>
  <p id="confirm-msg" class="dialog-msg">{message}</p>

  {#if requiredPhrase}
    <div class="phrase-row">
      <label class="phrase-label" for="confirm-input">
        Type <code class="phrase-code">{requiredPhrase}</code> to confirm
      </label>
      <input
        id="confirm-input"
        bind:this={inputEl}
        class="phrase-input"
        bind:value={typed}
        autocomplete="off"
        spellcheck={false}
        aria-label="Confirmation phrase"
      />
    </div>
  {/if}

  <div class="dialog-actions">
    <button class="cancel-btn" onclick={onCancel}>Cancel</button>
    <button
      class="confirm-btn"
      disabled={!canConfirm}
      onclick={onConfirm}
    >
      {confirmLabel}
    </button>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: 950;
  }

  .dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 960;
    width: min(400px, 90vw);
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 8px 40px var(--shadow);
    padding: 24px 24px 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .dialog-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
    margin: 0;
  }

  .dialog-msg {
    font-size: 13px;
    color: var(--muted);
    margin: 0;
    line-height: 1.5;
  }

  /* ── Typed phrase ─────────────────────────────────────── */
  .phrase-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .phrase-label {
    font-size: 12px;
    color: var(--muted);
  }

  .phrase-code {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 12px;
    background: var(--bg-b);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 5px;
    color: var(--text);
  }

  .phrase-input {
    width: 100%;
    height: 34px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-a);
    color: var(--text);
    font-size: 13px;
    font-family: ui-monospace, SFMono-Regular, monospace;
    box-sizing: border-box;
  }

  .phrase-input:focus {
    outline: none;
    border-color: var(--err);
  }

  /* ── Actions ──────────────────────────────────────────── */
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }

  .cancel-btn {
    height: 32px;
    padding: 0 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--card);
    color: var(--text);
    font-size: 13px;
    transition: background 0.1s;
  }

  .cancel-btn:hover { background: var(--bg-b); }

  .confirm-btn {
    height: 32px;
    padding: 0 16px;
    background: var(--err);
    color: white;
    border-radius: var(--radius-sm);
    font-size: 13px;
    font-weight: 500;
    transition: opacity 0.1s;
  }

  .confirm-btn:hover:not(:disabled) { opacity: 0.85; }

  .confirm-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
</style>
