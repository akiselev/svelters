<script>
  import { ListView } from '@svelters/lists';
  let { disabled = false } = $props();
  const items = ['الأرضي', 'الطاقة', 'البيانات', 'الساعة'].map((text, id) => ({ id, text }));
</script>

<box dir="rtl" padding={8} flex-grow={1}>
  <ListView anchor="subject" accessible-name="الشبكات" {items} key={(it) => it.id} row-height={24} flex-grow={1} enabled={!disabled}>
    {#snippet row(item)}
      <label>{item.text}</label>
    {/snippet}
  </ListView>
</box>
