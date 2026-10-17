<!-- kinds: QWidget QPushButton QLineEdit QLabel -->
<script>
  // Base-class facts (enabled, toolTip, size limits) shown on existing elements.
  let enabled = $state(true);
  let clicks = $state(0);
  $effect(() => {
    enabled = false;
  });
</script>

<box orientation="vertical" spacing="6">
  <label label="QWidget base props" />
  <button label="Disabled by enabled={false}" enabled={enabled} toolTip="Tooltip on a button" onclick={() => clicks++} />
  <button label="Enabled, min width 200" enabled={true} toolTip="Sized button" minimumWidth={200} maximumWidth={300} />
  <entry text="Entry with tooltip" toolTip="Tooltip on an entry" maximumWidth={260} />
  <label label="clicks: {clicks}" toolTip="Tooltip on a label" />
</box>
