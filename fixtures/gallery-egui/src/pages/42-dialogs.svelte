<script>
  // WG4b: native dialogs (rfd) as Promise-returning functions of `svelters:app/egui`. A cancelled
  // file dialog resolves with null; `{ signal }` aborts (rejects with `cancelled`).
  import { openFile, openFiles, saveFile, pickFolder, messageBox } from 'svelters:app/egui';

  let last = $state('nothing yet');
  const show = (what) => (v) => (last = `${what}: ${JSON.stringify(v)}`);
  const fail = (e) => (last = `error: ${e.code ?? e}`);
  const step = [{ name: 'STEP', extensions: ['step', 'stp'] }];
</script>

<box class="col">
  <box class="row">
    <button onclicked={() => openFile({ title: 'Open part', filters: step }).then(show('openFile'), fail)}>Open file…</button>
    <button onclicked={() => openFiles({ filters: step }).then(show('openFiles'), fail)}>Open files…</button>
    <button onclicked={() => saveFile({ fileName: 'board.kicad_pcb' }).then(show('saveFile'), fail)}>Save as…</button>
    <button onclicked={() => pickFolder({ title: 'Project folder' }).then(show('pickFolder'), fail)}>Folder…</button>
  </box>
  <box class="row">
    <button
      onclicked={() =>
        messageBox({ title: 'Unsaved changes', description: 'Save before closing?', level: 'warning', buttons: 'yes-no-cancel' }).then(
          show('messageBox'),
          fail,
        )}>Message box…</button
    >
    <button onclicked={() => messageBox({ title: 'Reload', labels: ['Reload', 'Keep mine'] }).then(show('labels'), fail)}>Custom buttons…</button>
  </box>
  <label>{last}</label>
</box>

<style>
  .col {
    gap: 8px;
  }
  .row {
    flex-direction: row;
    gap: 6px;
  }
</style>
