import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';

interface SessionState {
  active: boolean;
  expiresAt: number | null;
}

function createSessionStore() {
  const { subscribe, set } = writable<SessionState>({
    active: false,
    expiresAt: null,
  });

  /**
   * Open a session.
   * @param expiresAtMs - expiry timestamp in ms since epoch, or `null` for
   *   UntilQuit sessions that have no time-based expiry.
   */
  function open(expiresAtMs: number | null) {
    set({ active: true, expiresAt: expiresAtMs });
  }

  function close() {
    set({ active: false, expiresAt: null });
  }

  return { subscribe, open, close };
}

export const session = createSessionStore();

/**
 * Remaining seconds in the current session.
 * Returns `null` for UntilQuit sessions (no countdown), 0 when inactive.
 */
export const sessionSecondsLeft = derived(session, ($s) => {
  if (!$s.active) return 0;
  if ($s.expiresAt === null) return null; // UntilQuit — no countdown
  return Math.max(0, Math.round(($s.expiresAt - Date.now()) / 1000));
});

// Subscribe to the backend expiry event so the UI auto-locks.
// This runs once at module load time (Svelte app startup).
listen<void>('vault://session-expired', () => {
  session.close();
}).catch(console.error);
