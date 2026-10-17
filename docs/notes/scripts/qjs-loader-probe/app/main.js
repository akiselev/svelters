// Probe entry: everything below is resolved from node_modules and compiled on import.
import * as $ from 'svelte/internal/client';
import { untrack, flushSync } from 'svelte';
import * as $n from '@svelters/native-runtime';
import App from './App.svelte';
globalThis.__probe = { svelte: typeof $.state, untrack: typeof untrack, runtime: Object.keys($n).length, app: typeof App };
