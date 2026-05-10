import { writable } from 'svelte/store';
import type { Namespace } from '../api.js';

/** All namespaces loaded from the backend. */
export const namespaces = writable<Namespace[]>([]);

/** The currently active namespace id (empty string = nothing selected). */
export const activeNamespaceId = writable<string>('');
