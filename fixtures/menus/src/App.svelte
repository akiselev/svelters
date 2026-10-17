<script>
  // Menus and actions (X4). One tree for both toolkits: a menubar with a file menu (plain, check and
  // submenu items), a radio group, a toolbar and a context menu. The label shows what the handlers saw.
  let log = $state('');
  let autosave = $state(false);
  let small = $state(true);
  let large = $state(false);
  let recent = $state(['a.txt', 'b.txt']);
  const add = (s) => (log += s + ';');
</script>

<window title="Menus">
  <box orientation="vertical">
    <menubar>
      <menu label="File">
        <action label="Open" shortcut="Ctrl+O" onactivate={() => add('open')} />
        <action label="Autosave" checkable bind:checked={autosave} />
        <menuseparator />
        <menu label="Recent">
          {#each recent as r (r)}
            <action label={r} onactivate={() => add('recent:' + r)} />
          {/each}
        </menu>
        <action label="Quit" shortcut="Ctrl+Q" onactivate={() => add('quit')} />
      </menu>
      <menu label="Size">
        <action label="Small" group="size" bind:checked={small} />
        <action label="Large" group="size" bind:checked={large} />
      </menu>
    </menubar>
    <toolbar>
      <action icon="edit-copy" label="Copy" onactivate={() => add('copy')} />
    </toolbar>
    <button
      onclick={() => {
        recent = [...recent.slice(1), 'c.txt'].reverse();
      }}>Shuffle</button
    >
    <label>{log}|{autosave}|{small}|{large}</label>
    <button>
      Context
      <contextmenu>
        <action label="Paste" onactivate={() => add('paste')} />
      </contextmenu>
    </button>
  </box>
</window>
