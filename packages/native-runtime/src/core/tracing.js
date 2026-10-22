// `import '@svelters/native-runtime/core/tracing'`: what the compiler emits first in a component
// that uses `$inspect.trace` in dev (Svelte's `import 'svelte/internal/flags/tracing'`). Turns on the
// dependency recording `$.trace` reports, for every component, before any of them runs.
import { enable_tracing } from './svelte.js';

enable_tracing();
