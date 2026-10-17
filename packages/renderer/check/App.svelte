<script lang="ts">
  // Type-check target for svelte-check against packages/renderer/types (`pnpm check`).
  // A typo'd property (`titel`) or a wrongly typed one (`spacing="8"`) here makes it fail.
  let count = $state(0);
  let text = $state('');
</script>

<window title="Check">
  <box orientation="vertical" spacing={8}>
    <label>Count: {count}</label>
    <button onclick={() => count++}>Increment</button>
    <entry placeholder="Name" text={text}></entry>
  </box>
</window>
