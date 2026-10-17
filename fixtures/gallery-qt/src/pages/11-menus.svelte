<!-- kinds: QMenuBar QMenu QAction QToolBar -->
<script>
  // Menus and actions (X4): one declarative tree becomes native menus. Items insert, remove and
  // reorder reactively; checked states are bound; programmatic writes after mount must not echo.
  // A <menu> inside a widget that holds no menu items is that widget's context menu; separators are
  // actions with separator={true}.
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

<box-layout direction="top-to-bottom" spacing="6">
  <menu-bar>
    <menu title="File">
      <action text="Open" icon="document-open" shortcut="Ctrl+O" enabled={openEnabled} ontriggered={() => add('open')} />
      <action text="Autosave" checkable bind:checked={autosave} ontoggled={() => echoAutosave++} />
      <action separator={true} />
      <menu title="Recent">
        {#each recent as r (r)}
          <action text={r} ontriggered={() => add(r)} />
        {/each}
      </menu>
      {#if extra}
        <action text="Extra item" />
      {/if}
      <action text="Quit" shortcut="Ctrl+Q" ontriggered={() => add('quit')} />
    </menu>
    <menu title="Size">
      <action text="Small" group="size" bind:checked={small} ontoggled={() => echoSize++} />
      <action text="Large" group="size" bind:checked={large} ontoggled={() => echoSize++} />
    </menu>
  </menu-bar>
  <tool-bar>
    <action icon="edit-copy" text="Copy" ontriggered={() => add('copy')} />
    <action separator={true} />
    <action icon="format-text-bold" text="Bold" checkable />
  </tool-bar>
  <box-layout direction="left-to-right" spacing="6">
    <push-button text="Reverse recent" onclicked={() => (recent = [...recent].reverse())} />
    <push-button text="Drop first recent" onclicked={() => (recent = recent.slice(1))} />
    <push-button text="Toggle extra item" onclicked={() => (extra = !extra)} />
    <push-button text="Toggle Open enabled" onclicked={() => (openEnabled = !openEnabled)} />
  </box-layout>
  <label text="log: {log}" />
  <label text="autosave: {autosave}, small: {small}, large: {large}" />
  <label text="echo autosave: {echoAutosave}" />
  <label text="echo size: {echoSize}" />
  <push-button text="Right click for a context menu">
    <menu>
      <action text="Paste" ontriggered={() => add('paste')} />
      <action text="Select all" ontriggered={() => add('select-all')} />
    </menu>
  </push-button>
</box-layout>
