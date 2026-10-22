<script>
  // a runtime spread and <svelte:element>: the forwarded prop is an entry of the spread object
  let { tag, ...rest } = $props();
</script>

<box>
  <button {...rest} on:clicked />
  <svelte:element this={tag} on:clicked />
</box>
