<script>
  import { ListView } from '@svelters/lists';
  let { disabled = false } = $props();
  const items = Array.from({ length: 200 }, (_, i) => ({ id: i + 1, text: `Net ${i + 1}` }));
  let selected = $state(2);
</script>

<box padding={8} flex-grow={1}>
  <ListView anchor="subject" accessible-name="Nets" {items} key={(it) => it.id} bind:selected flex-grow={1} enabled={!disabled}>
    {#snippet row(item)}
      <label>{item.text}</label>
    {/snippet}
  </ListView>
</box>
