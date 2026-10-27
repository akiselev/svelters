<script>
  let { size = 'small', tone = 'plain', disabled = false } = $props();
</script>

<box class="cell">
  <button anchor="subject" class="b {size} {tone}" text={`Run ${size}`} enabled={!disabled} />
</box>

<style>
  .cell { padding: 8px; }
  .b:hover { background-color: #ff0000; }
  .large { min-width: 120px; }
</style>
