<script>
  // Qt twin of fixtures/menus (Qt vocabulary): a menu-bar with a file menu (plain, check and submenu
  // items), a radio group, a tool-bar and a context menu (a `menu` inside a widget that is not a menu,
  // menu bar or tool bar). The label shows what the handlers saw.
  let log = $state('');
  let autosave = $state(false);
  let small = $state(true);
  let large = $state(false);
  let recent = $state(['a.txt', 'b.txt']);
  const add = (s) => (log += s + ';');
  // Since R2 `bind:checked` observes `checked` (exact on Qt) instead of listening to `toggled`; the
  // empty `ontoggled` keeps the scripted `toggled` events delivered on the log backend and in the Qt
  // op-log replay. The op-log hosts cannot change `checked` from an `event` step, so the label keeps
  // the app's values there; the toolkit changes them for real (examples/counter-qt/tests/host_menus.rs).
  const toggled = () => {};
</script>

<main-window window-title="Menus">
  <box-layout direction="top-to-bottom">
    <menu-bar>
      <menu title="File">
        <action text="Open" shortcut="Ctrl+O" ontriggered={() => add('open')} />
        <action text="Autosave" checkable bind:checked={autosave} ontoggled={toggled} />
        <action separator={true} />
        <menu title="Recent">
          {#each recent as r (r)}
            <action text={r} ontriggered={() => add('recent:' + r)} />
          {/each}
        </menu>
        <action text="Quit" shortcut="Ctrl+Q" ontriggered={() => add('quit')} />
      </menu>
      <menu title="Size">
        <action text="Small" group="size" bind:checked={small} ontoggled={toggled} />
        <action text="Large" group="size" bind:checked={large} ontoggled={toggled} />
      </menu>
    </menu-bar>
    <tool-bar>
      <action icon="edit-copy" text="Copy" ontriggered={() => add('copy')} />
    </tool-bar>
    <push-button
      onclicked={() => {
        recent = [...recent.slice(1), 'c.txt'].reverse();
      }}>Shuffle</push-button
    >
    <label>S:{log}|{autosave}|{small}|{large}</label>
    <push-button>
      Context
      <menu>
        <action text="Paste" ontriggered={() => add('paste')} />
      </menu>
    </push-button>
  </box-layout>
</main-window>
