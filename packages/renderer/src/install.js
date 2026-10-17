// Side-effect module: publishes `__svelters_root` (the mount target) and
// `__svelters_dispatch(h, signal, args)`. The renderer reads `globalThis.__svelters_host` lazily.
import { root, dispatch } from './index.js';

if (!globalThis.__svelters_host) throw new Error('globalThis.__svelters_host is not defined');
globalThis.__svelters_root = root;
globalThis.__svelters_dispatch = dispatch;
