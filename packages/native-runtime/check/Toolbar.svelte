<script lang="ts">
  // FW1: a bare `on:clicked` forwards the `onclicked` callback prop; `Forward` declares it for
  // svelte-check, typed as <button>'s own `onclicked`.
  import type { Forward } from '../types/index';
  let { label }: { label: string } & Forward<'button', 'clicked'> = $props();
</script>

<button {label} on:clicked></button>
