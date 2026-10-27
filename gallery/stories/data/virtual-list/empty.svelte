<script>
  import { ListView } from '@svelters/lists';
  let { disabled = false } = $props();
</script>

<box padding={8} flex-grow={1}>
  <ListView anchor="subject" accessible-name="Nothing" items={[]} row-height={24} flex-grow={1} enabled={!disabled}>
    {#snippet row(item)}
      <label>{item}</label>
    {/snippet}
  </ListView>
</box>
