<script lang="ts">
  // Type-check target for svelte-check against packages/native-runtime/types (`pnpm check`).
  // A typo'd property (`titel`) or a wrongly typed one (`spacing="8"`) here makes it fail.
  let count = $state(0);
  let text = $state('');
  let draft = $state('');
  let cursor = $state(0);
  let win = $state<import('../types/native').NativeElement>();
</script>

<window title="Check" bind:this={win}>
  <box orientation="vertical" spacing={8}>
    <label>Count: {count}</label>
    <button onclicked={() => count++}>Increment</button>
    <entry placeholder-text="Name" bind:text></entry>
    <!-- R2: a snapshot binding and a read-only observable one (observe.d.ts) -->
    <entry bind:text|nonreactive={draft} bind:cursor-position={cursor}></entry>
    <button onclicked={() => win?.call('present')}>Present</button>
    <button class="primary" class:active={count > 1} style:color="red" style="padding: 4px" id="styled">Styled</button>
  </box>
</window>
