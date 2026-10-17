<!-- kinds: QComboBox QFontComboBox QListWidget -->
<script>
  // Selection: combo-box, font-combo-box, list-widget. Items are added with call() after mount; the
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

<box-layout direction="top-to-bottom" spacing="4">
  <combo-box bind:this={combo} bind:current-index={comboIndex} editable={false} max-visible-items={8}
            duplicates-enabled={false} placeholder-text="pick one"
            oncurrent-index-changed={() => { if (armed) echoCombo++; }} onactivated={() => activated++} />
  <label text="activated: {activated}" />
  <label text="echo current-index: {echoCombo}" />
  <font-combo-box />
  <list-widget bind:this={list} bind:current-row={listRow} alternating-row-colors={true} sorting-enabled={false}
              oncurrent-row-changed={() => { if (armed) echoList++; }}
              onitem-selection-changed={() => selected++} />
  <label text="selection changes: {selected}" />
  <label text="echo current-row: {echoList}" />
</box-layout>
