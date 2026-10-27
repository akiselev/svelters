<script>
  import { ListView } from '@svelters/lists';
  let { disabled = false } = $props();
  const items = Array.from({ length: 50 }, (_, i) => ({ id: i + 1, name: `Part ${i + 1}`, detail: i % 2 ? 'In stock: 1,200' : 'Back-ordered until next month' }));
</script>

<box padding={8} flex-grow={1}>
  <ListView anchor="subject" accessible-name="Parts" {items} key={(it) => it.id} row-height={40} flex-grow={1} enabled={!disabled}>
    {#snippet row(item)}
      <box gap={0}>
        <label font-weight={600}>{item.name}</label>
        <label elide="end">{item.detail}</label>
      </box>
    {/snippet}
  </ListView>
</box>
