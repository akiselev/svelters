<script lang="ts">
  // Type-check target for svelte-check against packages/renderer/types (`pnpm check`).
  // Lines under a `@ts-expect-error` comment must fail to type-check: if the types stop
  // rejecting them, svelte-check reports the unused directive.
  let count = $state(0);
  let text = $state('');
</script>

<window title="Check">
  <box orientation="vertical" spacing={8}>
    <label>Count: {count}</label>
    <button onclick={() => count++}>Increment</button>
    <entry placeholder="Name" text={text} />
    <!-- @ts-expect-error unknown property -->
    <label titel="typo">x</label>
    <!-- @ts-expect-error wrong property type -->
    <box spacing="8"></box>
    <!-- @ts-expect-error unknown element -->
    <nosuchwidget></nosuchwidget>
  </box>
</window>
