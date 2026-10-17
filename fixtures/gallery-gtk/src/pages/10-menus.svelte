<!-- kinds: Menubar Menu Action Toolbar Contextmenu Menuseparator -->
<script>
  // Menus and actions (X4): one declarative tree becomes native menus. Items insert, remove and
  // reorder reactively; checked states are bound; programmatic writes after mount must not echo.
  let log = $state('');
  let autosave = $state(false);
  let small = $state(false);
  let large = $state(false);
  let echoAutosave = $state(0);
  let echoSize = $state(0);
  let recent = $state(['a.txt', 'b.txt', 'c.txt']);
  let openEnabled = $state(true);
  let extra = $state(false);
  $effect(() => {
    autosave = true;
    small = true;
  });
  const add = (s) => (log += s + ' ');
</script>

<box orientation="vertical" spacing="6">
  <menu-bar>
    <menu label="File">
      <action label="Open" icon="document-open" shortcut="Ctrl+O" enabled={openEnabled} onactivate={() => add('open')} />
      <action label="Autosave" checkable bind:checked={autosave} ontoggled={() => echoAutosave++} />
      <menu-separator />
      <menu label="Recent">
        {#each recent as r (r)}
          <action label={r} onactivate={() => add(r)} />
        {/each}
      </menu>
      {#if extra}
        <action label="Extra item" />
      {/if}
      <action label="Quit" shortcut="Ctrl+Q" onactivate={() => add('quit')} />
    </menu>
    <menu label="Size">
      <action label="Small" group="size" bind:checked={small} ontoggled={() => echoSize++} />
      <action label="Large" group="size" bind:checked={large} ontoggled={() => echoSize++} />
    </menu>
  </menu-bar>
  <tool-bar>
    <action icon="edit-copy" label="Copy" onactivate={() => add('copy')} />
    <menu-separator />
    <action icon="format-text-bold" label="Bold" checkable />
  </tool-bar>
  <box orientation="horizontal" spacing="6">
    <button label="Reverse recent" onclicked={() => (recent = [...recent].reverse())} />
    <button label="Drop first recent" onclicked={() => (recent = recent.slice(1))} />
    <button label="Toggle extra item" onclicked={() => (extra = !extra)} />
    <button label="Toggle Open enabled" onclicked={() => (openEnabled = !openEnabled)} />
  </box>
  <label label="log: {log}" />
  <label label="autosave: {autosave}, small: {small}, large: {large}" />
  <label label="echo autosave: {echoAutosave}" />
  <label label="echo size: {echoSize}" />
  <button label="Right click for a context menu">
    <context-menu>
      <action label="Paste" onactivate={() => add('paste')} />
      <action label="Select all" onactivate={() => add('select-all')} />
    </context-menu>
  </button>
</box>
