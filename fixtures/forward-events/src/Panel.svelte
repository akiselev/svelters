<script>
  import Toolbar from './Toolbar.svelte';
  let { title } = $props();
</script>

<box>
  <label label={title} />
  <Toolbar on:clicked />
</box>
