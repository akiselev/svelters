// Side-effect module: publishes `__svelters_root` (the mount target) and
// `__svelters_dispatch(h, signal, args, deferred)`, `__svelters_dispatch_sync(h, signal, args)`, `__svelters_gone(h, reason)`
// and `__svelters_node(h)` (the host wraps handles in call/getProp results with it). The renderer reads
// `globalThis.__svelters_host` lazily.
import { root, dispatch, dispatchSync, gone, nodeFor } from './index.js';

if (!globalThis.__svelters_host) throw new Error('globalThis.__svelters_host is not defined');
globalThis.__svelters_root = root;
globalThis.__svelters_dispatch = dispatch;
globalThis.__svelters_dispatch_sync = dispatchSync;
globalThis.__svelters_gone = gone;
globalThis.__svelters_node = nodeFor;
