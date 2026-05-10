<script lang="ts">
  import { session } from '../../stores/session.js';
  import { unlockVault } from '../../api.js';

  // ── State ────────────────────────────────────────────────
  let authenticating = $state(false); // waiting for TouchID
  let opening = $state(false);        // door animation in progress
  let errorMsg = $state('');
  let visible = $state(true);

  // ── Login handler ────────────────────────────────────────
  async function handleLogin() {
    if (authenticating || opening) return;
    authenticating = true;
    errorMsg = '';

    try {
      // Calls Rust: TouchID prompt → Keychain key → session token.
      // The door stays closed until this resolves.
      const status = await unlockVault();

      authenticating = false;
      opening = true; // start door animation only after successful auth

      if (status.active) {
        session.open(status.expires_at_ms);
      }

      // Wait for the CSS door-open transition to finish (750 ms)
      await new Promise<void>((r) => setTimeout(r, 750));
      visible = false;
    } catch (err: unknown) {
      authenticating = false;
      opening = false;
      const msg = err instanceof Error ? err.message : String(err);
      errorMsg = msg || 'Authentication failed. Try again.';
      setTimeout(() => (errorMsg = ''), 4000);
    }
  }
</script>

{#if visible}
  <!-- Door overlay — sits above AppShell until opened -->
  <div class="door-stage" aria-label="Vaultor login">

    <!-- Left panel -->
    <div class="door door-left" class:opening>
      <div class="bolt bolt-tl"></div>
      <div class="bolt bolt-bl"></div>
      <div class="panel-lines">
        {#each [0,1,2,3,4] as _}
          <div class="panel-line"></div>
        {/each}
      </div>
    </div>

    <!-- Right panel -->
    <div class="door door-right" class:opening>
      <div class="bolt bolt-tr"></div>
      <div class="bolt bolt-br"></div>
      <div class="panel-lines">
        {#each [0,1,2,3,4] as _}
          <div class="panel-line"></div>
        {/each}
      </div>
    </div>

    <!-- Center seam overlay — logo + button -->
    <div class="seam-content" class:fading={opening}>
      <button
        class="logo-btn"
        onclick={handleLogin}
        aria-label="Open vault"
        disabled={authenticating || opening}
      >
        <!-- Vault logo SVG -->
        <svg
          class="logo-svg"
          viewBox="0 0 100 100"
          xmlns="http://www.w3.org/2000/svg"
          aria-hidden="true"
        >
          <!-- Outer ring -->
          <circle cx="50" cy="50" r="44" fill="none" stroke="var(--brand-dark)" stroke-width="4"/>
          <!-- Inner ring -->
          <circle cx="50" cy="50" r="30" fill="none" stroke="var(--brand-mid)" stroke-width="2.5"/>
          <!-- Centre fill -->
          <circle cx="50" cy="50" r="27" fill="var(--bg-a)" fill-opacity="0.7"/>
          <!-- Letter V -->
          <text
            x="50"
            y="63"
            text-anchor="middle"
            font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
            font-size="34"
            font-weight="700"
            fill="var(--brand-dark)"
          >V</text>
          <!-- Handle spoke -->
          <line x1="82" y1="50" x2="94" y2="50" stroke="var(--brand-dark)" stroke-width="3.5" stroke-linecap="round"/>
        </svg>
      </button>

      <button
        class="login-btn"
        onclick={handleLogin}
        disabled={authenticating || opening}
      >
        {#if authenticating}
          Authenticating…
        {:else if opening}
          Opening…
        {:else}
          Login
        {/if}
      </button>

      {#if errorMsg}
        <p class="error-msg" role="alert">{errorMsg}</p>
      {/if}
    </div>

  </div>
{/if}

<style>
  /* ── Stage ──────────────────────────────────────────────── */
  .door-stage {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    overflow: hidden;
  }

  /* ── Door panels ─────────────────────────────────────────── */
  .door {
    position: absolute;
    top: 0;
    height: 100%;
    width: 50%;
    background: linear-gradient(160deg, var(--bg-a) 0%, var(--bg-b) 55%, var(--brand) 100%);
    display: flex;
    align-items: center;
    overflow: hidden;
    transition: transform 0.75s cubic-bezier(0.7, 0, 0.3, 1);
    will-change: transform;
  }

  .door-left {
    left: 0;
    border-right: 2px solid var(--brand-mid);
    box-shadow: inset -8px 0 24px var(--shadow), 4px 0 16px var(--shadow);
    justify-content: flex-end;
    padding-right: 24px;
  }

  .door-right {
    right: 0;
    border-left: 2px solid var(--brand-mid);
    box-shadow: inset 8px 0 24px var(--shadow), -4px 0 16px var(--shadow);
    justify-content: flex-start;
    padding-left: 24px;
  }

  .door-left.opening  { transform: translateX(-100%); }
  .door-right.opening { transform: translateX(100%); }

  /* ── Bolts (decorative vault corners) ───────────────────── */
  .bolt {
    position: absolute;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--brand-dark);
    box-shadow: 0 2px 6px var(--shadow), inset 0 1px 2px rgba(255,255,255,0.3);
  }

  .bolt-tl { top: 32px;  left: 24px; }
  .bolt-bl { bottom: 32px; left: 24px; }
  .bolt-tr { top: 32px;  right: 24px; }
  .bolt-br { bottom: 32px; right: 24px; }

  /* ── Decorative panel lines ──────────────────────────────── */
  .panel-lines {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    flex-direction: column;
    gap: 14px;
    opacity: 0.25;
  }

  .door-left .panel-lines  { left: 48px; }
  .door-right .panel-lines { right: 48px; }

  .panel-line {
    width: 56px;
    height: 3px;
    border-radius: 2px;
    background: var(--brand-dark);
  }

  /* ── Centre seam content ────────────────────────────────── */
  .seam-content {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    z-index: 10;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 20px;
    transition: opacity 0.35s ease;
  }

  .seam-content.fading {
    opacity: 0;
    pointer-events: none;
  }

  /* ── Logo button ─────────────────────────────────────────── */
  .logo-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--card);
    border: 2px solid var(--border);
    border-radius: 50%;
    padding: 12px;
    box-shadow: 0 4px 24px var(--shadow);
    transition: transform 0.2s ease, box-shadow 0.2s ease;
  }

  .logo-btn:hover:not(:disabled) {
    transform: scale(1.06);
    box-shadow: 0 8px 32px var(--shadow);
  }

  .logo-btn:disabled {
    cursor: default;
    opacity: 0.8;
  }

  .logo-svg {
    width: 96px;
    height: 96px;
  }

  /* ── Login button ────────────────────────────────────────── */
  .login-btn {
    padding: 10px 36px;
    background: var(--brand-dark);
    color: white;
    border-radius: var(--radius);
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.03em;
    box-shadow: 0 2px 12px var(--shadow);
    transition: background 0.15s ease, transform 0.15s ease;
  }

  .login-btn:hover:not(:disabled) {
    background: var(--brand-mid);
    transform: translateY(-1px);
  }

  .login-btn:disabled {
    cursor: default;
    opacity: 0.7;
  }

  /* ── Error message ───────────────────────────────────────── */
  .error-msg {
    color: var(--err);
    font-size: 13px;
    background: var(--err-bg);
    border: 1px solid var(--err-border);
    border-radius: var(--radius-sm);
    padding: 6px 14px;
  }
</style>
