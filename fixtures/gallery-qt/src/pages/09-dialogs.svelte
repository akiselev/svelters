<!-- kinds: QLabel QDialog QMessageBox QInputDialog QProgressDialog QErrorMessage QFileDialog QColorDialog QFontDialog -->
<script>
  // Dialogs are toplevel kinds. They are declared in `if` blocks inside the window: the renderer
  // makes each one transient for the enclosing window and opens it window-modal with `open()`
  // (never `exec()`); `show="modeless"` would use `show()`, `show="none"` leaves it hidden for
  // `bind:this` + `.call('open')`. Leaving the block destroys the dialog. `finished` reports how
  // it ended; a dialog closed with its window button is reported natively as closed.
  const kinds = ['messagebox', 'inputdialog', 'progressdialog', 'colordialog', 'fontdialog', 'filedialog', 'dialog'];
  let open = $state({ messagebox: true });
  let finished = $state(0);
  let last = $state('none');

  const done = (kind) => (code) => {
    open[kind] = false;
    finished++;
    last = `${kind} ${code}`;
  };
  function openAll() {
    for (const k of kinds) open[k] = true;
  }
</script>

<box orientation="vertical" spacing="6">
  <button label="open all dialogs" onclick={openAll} />
  <box orientation="horizontal" spacing="6">
    {#each kinds as k}
      <button label={k} onclick={() => (open[k] = true)} />
    {/each}
  </box>
  <label label="finished: {finished}, last: {last}" />

  {#if open.messagebox}
    <messagebox text="Save changes?" informativeText="Opened with open(), not exec()" onfinished={done('messagebox')} />
  {/if}
  {#if open.inputdialog}
    <inputdialog labelText="Name" textValue="svelters" onfinished={done('inputdialog')} />
  {/if}
  {#if open.progressdialog}
    <progressdialog labelText="Working" minimum={0} maximum={100} value={40} onfinished={done('progressdialog')} />
  {/if}
  {#if open.colordialog}
    <colordialog onfinished={done('colordialog')} />
  {/if}
  {#if open.fontdialog}
    <fontdialog onfinished={done('fontdialog')} />
  {/if}
  {#if open.filedialog}
    <filedialog onfinished={done('filedialog')} />
  {/if}
  {#if open.dialog}
    <dialog title="A plain dialog" onfinished={done('dialog')}>
      <label label="children of a dialog are its layout contents" />
    </dialog>
  {/if}
</box>
