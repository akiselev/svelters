<script>
  import { getContext } from 'svelte';

  let { item, index } = $props();
  // Local to the row slot, not to the item: kept when the slot is rebound to another item.
  let clicks = $state(0);
  const tag = getContext('tag');
</script>

<box-layout direction="left-to-right" spacing="6">
  <label>{`${tag} ${index}: ${item.text}`}</label>
  <push-button onclicked={() => clicks++}>{`clicks ${clicks}`}</push-button>
</box-layout>
