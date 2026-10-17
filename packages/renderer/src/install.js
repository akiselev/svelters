// Side-effect module: publishes `__svelters_root` (the mount target) and
// `__svelters_dispatch(h, signal, args, deferred)`, `__svelters_dispatch_sync(h, signal, args)` and `__svelters_gone(h, reason)`. The renderer reads `globalThis.__svelters_host` lazily.
import { root, dispatch, dispatchSync, gone } from './index.js';

if (!globalThis.__svelters_host) throw new Error('globalThis.__svelters_host is not defined');
globalThis.__svelters_root = root;
globalThis.__svelters_dispatch = dispatch;
globalThis.__svelters_dispatch_sync = dispatchSync;
globalThis.__svelters_gone = gone;
