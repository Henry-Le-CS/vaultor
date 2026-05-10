import { writable } from 'svelte/store';
import type { SecretMeta } from '../api.js';

/** All secrets for the currently active namespace. */
export const secrets = writable<SecretMeta[]>([]);
