<!-- kinds: menu-bar menu action menu-separator context-menu menu-button popup modal inner-window -->
<script>
  // WG2a gallery page (plans/milestones/WG2a-menus-popups.md): the egui menu family in the X4
  // vocabulary of GTK and Qt (menu-bar, menu, action, menu-separator, context-menu), menu-button,
  // and the overlays (popup, modal, inner-window). The status label shows what the handlers saw.
  let log = $state('');
  let autosave = $state(false);
  let small = $state(true);
  let large = $state(false);
  let more = $state(false);
  let picking = $state(false);
  let filter = $state('');
  let confirming = $state(false);
  let inspector = $state(false);
  const add = (s) => (log += s + ';');
</script>

<window title="Menus and popups" default-width={520} default-height={400}>
  <box class="page">
    <menu-bar>
      <menu label="File">
        <action label="Open" shortcut="Ctrl+O" onactivate={() => add('open')} />
        <action label="Autosave" checkable bind:checked={autosave} />
        <menu-separator />
        <menu label="Recent">
          <action label="a.txt" onactivate={() => add('recent:a')} />
        </menu>
        <action label="Quit" shortcut="Ctrl+Q" onactivate={() => add('quit')} />
      </menu>
      <menu label="Size">
        <action label="Small" group="size" bind:checked={small} />
        <action label="Large" group="size" bind:checked={large} />
      </menu>
    </menu-bar>
    <box class="row">
      <menu-button label="More" bind:active={more}>
        <action label="Copy" onactivate={() => add('copy')} />
        <action label="Paste" onactivate={() => add('paste')} />
      </menu-button>
      <button onclicked={() => (picking = !picking)}
        >Pick<popup bind:open={picking} class="pop">
          <label>Filter</label>
          <text-edit hint-text="Filter text" bind:text={filter} />
        </popup>
      </button>
      <button onclicked={() => (confirming = true)}>Delete</button>
      <button onclicked={() => (inspector = true)}
        >Show inspector<context-menu>
          <action label="Hide inspector" onactivate={() => (inspector = false)} />
        </context-menu>
      </button>
    </box>
    <label>S:{log}|{autosave}|{small}|{large}|{more}|{filter}|{inspector}</label>
    <modal bind:open={confirming} title="Delete the file?" class="pop">
      <label>It cannot be undone.</label>
      <button onclicked={() => { add('deleted'); confirming = false; }}>Confirm</button>
    </modal>
    <inner-window bind:open={inspector} title="Inspector" default-width={220} default-height={120} default-x={280} default-y={230}>
      <label>Selection: none</label>
    </inner-window>
  </box>
</window>

<style>
  .page {
    gap: 8px;
  }
  .row {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }
  .pop {
    padding: 6px;
    gap: 6px;
  }
</style>
