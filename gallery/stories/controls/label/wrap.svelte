<script>
  let { mode = 'wrap', disabled = false } = $props();
</script>

<box padding={12} width={300}>
  <label anchor="subject" wrap-mode={mode} enabled={!disabled}>
    The design rule check found clearance violations on three nets; open the report to see each one on the board.
  </label>
</box>
