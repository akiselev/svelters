<!-- kinds: QComboBox QFontComboBox QListWidget -->
<script>
  // Selection: combobox, fontcombobox, listwidget. Items are added with call() after mount; the
  // bound indices are written once after that (echo suppression: the echo labels must stay 0).
  // QTreeWidget, QTableWidget and QButtonGroup are not exposed (see their override files).
  let combo = $state(null);
  let list = $state(null);
  let comboIndex = $state(0);
  let listRow = $state(0);
  let echoCombo = $state(0);
  let echoList = $state(0);
  let activated = $state(0);
  let selected = $state(0);
  let armed = false;
  $effect(() => {
    if (!combo || !list) return;
    for (const t of ['red', 'green', 'blue']) {
      combo.call('addItem', t, null);
      list.call('addItem', t);
    }
    setTimeout(() => { armed = true; comboIndex = 1; listRow = 2; }, 0);
  });
</script>

<box orientation="vertical" spacing="4">
  <combobox bind:this={combo} bind:currentIndex={comboIndex} editable={false} maxVisibleItems={8}
            duplicatesEnabled={false} placeholderText="pick one"
            onchanged={() => { if (armed) echoCombo++; }} onactivated={() => activated++} />
  <label label="activated: {activated}" />
  <label label="echo currentIndex: {echoCombo}" />
  <fontcombobox />
  <listwidget bind:this={list} bind:currentRow={listRow} alternatingRowColors={true} sortingEnabled={false}
              onrow-changed={() => { if (armed) echoList++; }}
              onselection-changed={() => selected++} />
  <label label="selection changes: {selected}" />
  <label label="echo currentRow: {echoList}" />
</box>
