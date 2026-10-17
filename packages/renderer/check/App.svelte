<script lang="ts">
  // Type-check target for svelte-check against packages/renderer/types (`pnpm check`).
  // A typo'd property (`titel`) or a wrongly typed one (`spacing="8"`) here makes it fail.
  let count = $state(0);
  let text = $state('');
  let win = $state<import('../types/native').NativeElement>();
</script>

<window title="Check" bind:this={win}>
  <box orientation="vertical" spacing={8}>
    <label>Count: {count}</label>
    <button onclick={() => count++}>Increment</button>
    <entry placeholder="Name" bind:text></entry>
    <button onclick={() => win?.call('present')}>Present</button>
    <button class="primary" class:active={count > 1} style:color="red" style="padding: 4px" id="styled">Styled</button>
  </box>
</window>
