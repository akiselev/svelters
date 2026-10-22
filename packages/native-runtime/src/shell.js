// `svelters:shell` (also `@svelters/native-runtime/shell`), DX2: hand URLs and files to the system.
//
//   import { openUrl, openPath, revealInFolder } from 'svelters:shell';
//   await openPath('/home/me/boards/main.kicad_pcb');   // the system's default application
//   await revealInFolder('/home/me/out/gerbers.zip');   // the file manager, the file selected
//
// Paths are absolute and must exist. The host's shell is `HostConfig::platform` (svelters-host
// `platform.rs`): the system one in a desktop runner; in tests a recording double, and in a headless
// host none (every call rejects). Every call returns a Promise; a failure rejects it.

const host = () => {
  const h = globalThis.__svelters_host;
  if (!h || typeof h.shellOpen !== 'function') throw new Error('svelters:shell: this host has no shell');
  return h;
};

const call = (what, target) => {
  try {
    host().shellOpen(what, String(target));
    return Promise.resolve();
  } catch (e) {
    return Promise.reject(e);
  }
};

/** Opens `url` with its handler (the browser, the mail client). */
export const openUrl = (url) => call('url', url);
/** Opens the file or directory at `path` with the system's default application. */
export const openPath = (path) => call('path', path);
/** Shows `path` selected in the file manager. */
export const revealInFolder = (path) => call('reveal', path);

export const shell = { openUrl, openPath, revealInFolder };
export default shell;
