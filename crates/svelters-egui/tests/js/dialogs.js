// WG4b: the dialog exports of `svelters:app/egui`, driven by `tests/wg4b.rs` against a fake
// `DialogBackend`. Each result is logged as one console line.
import { openFile, openFiles, saveFile, pickFolder, messageBox } from 'svelters:app/egui';

const log = (...xs) => console.log(xs.map((x) => JSON.stringify(x)).join(' '));

(async () => {
  log('open', await openFile({ title: 'Open part', filters: [{ name: 'STEP', extensions: ['step', 'stp'] }] }));
  log('open-none', await openFile());
  log('many', await openFiles({ directory: '/parts' }));
  log('save', await saveFile({ fileName: 'board.kicad_pcb' }));
  log('folder', await pickFolder({}));
  log('box', await messageBox({ title: 'Unsaved changes', description: 'Save?', level: 'warning', buttons: 'yes-no-cancel' }));
  log('labels', await messageBox({ labels: ['Keep', 'Discard'] }));
  // the fake keeps this one pending: an abort rejects at once with `cancelled`
  const ac = new AbortController();
  const pending = messageBox({ title: 'pending' }, { signal: ac.signal });
  ac.abort();
  try {
    await pending;
    log('not-aborted');
  } catch (e) {
    log('aborted', e.domain, e.code);
  }
  log('done');
})().catch((e) => log('error', String(e), e && e.code));
