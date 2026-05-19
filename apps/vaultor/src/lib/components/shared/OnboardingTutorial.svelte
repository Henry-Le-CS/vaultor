<script lang="ts">
  import { setTutorialSeen } from '../../api.js';

  interface Props {
    onClose: () => void;
  }
  let { onClose }: Props = $props();

  const steps = [
    {
      title: 'Welcome to Vaultor',
      body: 'Your secrets are encrypted on your Mac and locked behind your fingerprint. No cloud, no accounts, no subscriptions.',
      icon: 'lock',
    },
    {
      title: 'Create a Namespace',
      body: 'Namespaces are folders for your secrets. Click the + button at the bottom of the left sidebar to create one — try "Work", "Personal", or "Dev".',
      icon: 'folder',
    },
    {
      title: 'Add Secrets',
      body: 'Click "New" in the middle panel to add a secret. Choose key-value for passwords and API keys, or file for SSH keys and certificates.',
      icon: 'key',
    },
    {
      title: 'View & Copy',
      body: 'Values are hidden by default. Click the eye icon to reveal, or the copy icon to copy directly to your clipboard without showing it.',
      icon: 'eye',
    },
    {
      title: 'Settings & Backup',
      body: 'Click the gear icon in the sidebar to configure session timeout, move your vault file, or connect a private git repository for encrypted backup.',
      icon: 'gear',
    },
  ];

  let currentStep = $state(0);
  const isLast = $derived(currentStep === steps.length - 1);

  function next() {
    if (isLast) {
      finish();
    } else {
      currentStep++;
    }
  }

  function prev() {
    if (currentStep > 0) currentStep--;
  }

  async function finish() {
    try { await setTutorialSeen(); } catch { /* non-fatal */ }
    onClose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') finish();
    if (e.key === 'ArrowRight' || e.key === 'Enter') next();
    if (e.key === 'ArrowLeft') prev();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="tutorial-backdrop" role="dialog" aria-modal="true" aria-label="Onboarding tutorial">
  <div class="tutorial-card">
    <!-- Step icon -->
    <div class="tutorial-icon">
      {#if steps[currentStep].icon === 'lock'}
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <rect x="5" y="11" width="14" height="10" rx="2" stroke="currentColor" stroke-width="1.5"/>
          <path d="M8 11V7a4 4 0 0 1 8 0v4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
      {:else if steps[currentStep].icon === 'folder'}
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z" stroke="currentColor" stroke-width="1.5"/>
        </svg>
      {:else if steps[currentStep].icon === 'key'}
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <circle cx="8" cy="15" r="4" stroke="currentColor" stroke-width="1.5"/>
          <path d="M11.3 11.7 20 3m0 0h-4m4 0v4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      {:else if steps[currentStep].icon === 'eye'}
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z" stroke="currentColor" stroke-width="1.5"/>
          <circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.5"/>
        </svg>
      {:else}
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path stroke-linecap="round" stroke-linejoin="round" stroke="currentColor" stroke-width="1.5"
            d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0 1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94H9.75c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z"/>
          <path stroke-linecap="round" stroke-linejoin="round" stroke="currentColor" stroke-width="1.5"
            d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"/>
        </svg>
      {/if}
    </div>

    <!-- Content -->
    <h2 class="tutorial-title">{steps[currentStep].title}</h2>
    <p class="tutorial-body">{steps[currentStep].body}</p>

    <!-- Step dots -->
    <div class="tutorial-dots">
      {#each steps as _, i}
        <button
          class="tutorial-dot"
          class:active={i === currentStep}
          aria-label="Go to step {i + 1}"
          onclick={() => (currentStep = i)}
        ></button>
      {/each}
    </div>

    <!-- Navigation -->
    <div class="tutorial-nav">
      <button class="tutorial-skip" onclick={finish}>
        Skip
      </button>
      <div class="tutorial-nav-right">
        {#if currentStep > 0}
          <button class="tutorial-btn tutorial-btn--secondary" onclick={prev}>
            Back
          </button>
        {/if}
        <button class="tutorial-btn tutorial-btn--primary" onclick={next}>
          {isLast ? 'Get Started' : 'Next'}
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .tutorial-backdrop {
    position: fixed;
    inset: 0;
    z-index: 9999;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(61, 53, 87, 0.55);
    backdrop-filter: blur(4px);
    animation: fadeIn 0.25s ease;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .tutorial-card {
    background: var(--card);
    border-radius: 16px;
    box-shadow: 0 12px 48px rgba(0, 0, 0, 0.18);
    padding: 40px 36px 28px;
    max-width: 420px;
    width: 90%;
    text-align: center;
    animation: slideUp 0.3s ease;
  }

  @keyframes slideUp {
    from { transform: translateY(20px); opacity: 0; }
    to { transform: translateY(0); opacity: 1; }
  }

  .tutorial-icon {
    width: 56px;
    height: 56px;
    margin: 0 auto 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-b);
    border-radius: 14px;
    color: var(--brand-dark);
  }

  .tutorial-icon svg {
    width: 28px;
    height: 28px;
  }

  .tutorial-title {
    font-size: 20px;
    font-weight: 700;
    color: var(--text);
    margin-bottom: 10px;
  }

  .tutorial-body {
    font-size: 14px;
    line-height: 1.6;
    color: var(--muted);
    margin-bottom: 24px;
    min-height: 3.2em;
  }

  .tutorial-dots {
    display: flex;
    gap: 8px;
    justify-content: center;
    margin-bottom: 24px;
  }

  .tutorial-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--border);
    transition: background 0.2s, transform 0.2s;
    padding: 0;
  }

  .tutorial-dot.active {
    background: var(--brand-dark);
    transform: scale(1.25);
  }

  .tutorial-nav {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .tutorial-nav-right {
    display: flex;
    gap: 8px;
  }

  .tutorial-skip {
    font-size: 13px;
    color: var(--muted);
    padding: 6px 12px;
    border-radius: var(--radius);
    transition: color 0.15s;
  }

  .tutorial-skip:hover {
    color: var(--text);
  }

  .tutorial-btn {
    padding: 8px 20px;
    border-radius: var(--radius);
    font-size: 13px;
    font-weight: 600;
    transition: background 0.15s, color 0.15s;
  }

  .tutorial-btn--secondary {
    background: var(--bg-b);
    color: var(--text);
  }

  .tutorial-btn--secondary:hover {
    background: var(--border);
  }

  .tutorial-btn--primary {
    background: var(--brand-dark);
    color: white;
  }

  .tutorial-btn--primary:hover {
    background: var(--brand-mid);
  }
</style>
