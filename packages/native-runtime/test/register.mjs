// Node equivalent of the Vite alias in vite.js: resolve the effects specifier to the hidden file.
import { register } from 'node:module';

register('./alias-hook.mjs', import.meta.url);
