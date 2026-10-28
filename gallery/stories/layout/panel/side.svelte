<script>
  let { side = 'left' } = $props();
  const row = side === 'left' || side === 'right';
</script>

<box flex-grow={1} flex-direction={row ? (side === 'left' ? 'row' : 'row-reverse') : (side === 'top' ? 'column' : 'column-reverse')}>
  <panel anchor="subject" {side} size={row ? 140 : 64} resizable>
    <box padding={8} gap={4}>
      <label>Outline</label>
      <label>U1 — STM32G431</label>
    </box>
  </panel>
  <box class="editor" padding={8} flex-grow={1}><label>Board</label></box>
</box>

<style>
  .editor {
    background-color: var(--sv-surface);
  }
</style>
