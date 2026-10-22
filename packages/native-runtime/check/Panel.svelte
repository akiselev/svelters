<script lang="ts">
  // FW1: a bare `on:clicked` on a child component forwards the child's callback prop;
  // `ForwardFrom` types it as the child declares it.
  import type { ForwardFrom } from '../types/index';
  import Toolbar from './Toolbar.svelte';
  let { title }: { title: string } & ForwardFrom<typeof Toolbar, 'clicked'> = $props();
</script>

<box>
  <label label={title}></label>
  <Toolbar label="Go" on:clicked />
</box>
