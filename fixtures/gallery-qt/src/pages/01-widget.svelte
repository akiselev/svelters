<!-- kinds: QWidget QPushButton QLineEdit QLabel -->
<script>
  // Base-class facts (enabled, tool-tip, size limits) shown on existing elements.
  let enabled = $state(true);
  let clicks = $state(0);
  $effect(() => {
    enabled = false;
  });
</script>

<box-layout direction="top-to-bottom" spacing="6">
  <label text="QWidget base props" />
  <push-button text="Disabled by enabled={false}" enabled={enabled} tool-tip="Tooltip on a button" onclicked={() => clicks++} />
  <push-button text="Enabled, min width 200" enabled={true} tool-tip="Sized button" minimum-width={200} maximum-width={300} />
  <line-edit text="Line edit with tooltip" tool-tip="Tooltip on a line edit" maximum-width={260} />
  <label text="clicks: {clicks}" tool-tip="Tooltip on a label" />
</box-layout>
