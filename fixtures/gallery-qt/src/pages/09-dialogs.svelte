<!-- kinds: QLabel QDialog QMessageBox QInputDialog QProgressDialog QErrorMessage QFileDialog QColorDialog QFontDialog -->
<script>
  // Dialogs are toplevel kinds. They are declared in `if` blocks inside the window: the renderer
  // makes each one transient for the enclosing window and opens it window-modal with `open()`
  // (never `exec()`); `show="modeless"` would use `show()`, `show="none"` leaves it hidden for
  // `bind:this` + `.call('open')`. Leaving the block destroys the dialog. `finished` reports how
  // it ended; a dialog closed with its window button is reported natively as closed.
  const kinds = ['message-box', 'input-dialog', 'progress-dialog', 'color-dialog', 'font-dialog', 'file-dialog', 'dialog'];
  let open = $state(Object.fromEntries(kinds.map((k) => [k, true])));
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

<box-layout direction="top-to-bottom" spacing="6">
  <push-button text="open all dialogs" onclicked={openAll} />
  <box-layout direction="left-to-right" spacing="6">
    {#each kinds as k}
      <push-button text={k} onclicked={() => (open[k] = true)} />
    {/each}
  </box-layout>
  <label text="finished: {finished}, last: {last}" />

  {#if open['message-box']}
    <message-box text="Save changes?" informative-text="Opened with open(), not exec()" onfinished={done('message-box')} />
  {/if}
  {#if open['input-dialog']}
    <input-dialog label-text="Name" text-value="svelters" onfinished={done('input-dialog')} />
  {/if}
  {#if open['progress-dialog']}
    <progress-dialog label-text="Working" minimum={0} maximum={100} value={40} onfinished={done('progress-dialog')} />
  {/if}
  {#if open['color-dialog']}
    <color-dialog onfinished={done('color-dialog')} />
  {/if}
  {#if open['font-dialog']}
    <font-dialog onfinished={done('font-dialog')} />
  {/if}
  {#if open['file-dialog']}
    <file-dialog onfinished={done('file-dialog')} />
  {/if}
  {#if open.dialog}
    <dialog window-title="A plain dialog" onfinished={done('dialog')}>
      <label text="children of a dialog are its layout contents" />
    </dialog>
  {/if}
</box-layout>
