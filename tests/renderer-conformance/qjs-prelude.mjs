// Minimal host globals for quickjs-ng (what a native svelters host would provide).
import * as os from 'qjs:os';
globalThis.setTimeout ??= os.setTimeout;
globalThis.clearTimeout ??= os.clearTimeout;
globalThis.console ??= { log: print, warn: print, error: print };
