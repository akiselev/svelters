<script lang="ts">
  import { doubled } from './store.svelte.js';
  interface Props { start?: number }
  let { start = 0 }: Props = $props();
  let count: number = $state(start);
  const twice = $derived(doubled(count));
</script>

<window title="Counter">
  <box orientation="vertical" spacing="8">
    <label>Count: {count} (x2 = {twice})</label>
    <button onclick={() => count++}>Increment</button>
  </box>
</window>
