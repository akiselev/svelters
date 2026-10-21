<!-- backends: gtk; strict: gtk -->
<script>
  import { TextTag } from 'svelters:gtk/gtk';
  const tag = new TextTag();
  let tags = $state([{ font: 'a' }]);
  // shadowed by a local of the derived body: a plain object, not the wrapper (silent)
  let local = $derived.by(() => {
    const tag = { font: 'x' };
    return tag.font;
  });
  // shadowed by a parameter of the derived function (silent)
  let param = $derived.by(function (tag = { font: 'p' }) {
    return tag.font;
  });
  // not shadowed: the wrapper (warns)
  let real = $derived.by(() => {
    const other = 1;
    return tag.font + other;
  });
</script>
<window>
  <box orientation="vertical">
    {#each tags as tag}
      <label label={tag.font} />
    {/each}
    {#each tags as item, tag}
      <label label="{item.font} {tag.font}" />
    {/each}
    {#snippet row(tag)}
      <label label={tag.font} />
    {/snippet}
    {@render row({ font: 'r' })}
    {#if tags.length}
      {@const tag = tags[0]}
      <label label={tag.font} />
    {/if}
    {#each tags as item}
      <label label="{item.font} {tag.font}" />
    {/each}
    <label label={tag.font} />
  </box>
</window>
