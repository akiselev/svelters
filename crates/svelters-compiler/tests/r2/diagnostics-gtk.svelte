<!-- backends: gtk; strict: gtk -->
<script>
  import { TextTag } from 'svelters:gtk/gtk';
  import { untrack } from 'svelte';
  import Child from './Child.svelte';
  const tag = new TextTag();
  let x = $state(0), s = $state(''), el = $state(null), f = $state(false);
  let font = $derived(tag.font);
  let quiet = $derived(untrack(() => tag.font));
</script>
<window>
  <box orientation="vertical">
    <entry bind:text-length={x} />
    <entry bind:css-name={s} />
    <header-bar bind:title={s} />
    <header-bar bind:title|nonreactive={s} />
    <entry bind:has-focus={f} />
    <entry bind:progress-pulse-step={x} />
    <entry bind:text|bogus={s} />
    <entry bind:text|nonreactive|nonreactive={s} />
    <entry bind:this={el} />
    <label bind:this|nonreactive={el} label="{tag.font} {tag.name}" />
    <button onclicked={() => console.log(tag.font)} label="ok" />
    <Child bind:value|nonreactive={s} />
  </box>
</window>
