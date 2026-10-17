// Side-effect module: installs DOM globals bound to `globalThis.__svelters_host`, plus
// `__svelters_root` (mount target) and `__svelters_dispatch(h, signal, args)`.
// Must be the first import of an app entry so the globals exist before Svelte mounts.
import { installDom } from './index.js';

if (!globalThis.__svelters_host) throw new Error('globalThis.__svelters_host is not defined');
installDom(globalThis.__svelters_host);
