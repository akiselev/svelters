<!-- Named native child slots, validated at compile time against the backend spec.
     GTK: window.titlebar takes a headerbar; headerbar has start/end slots. Qt has no headerbar. -->
<script>
  let saved = $state(0);
</script>

<window title="Slots">
  <headerbar slot="titlebar">
    <button slot="start" onclick={() => saved++}>Save</button>
    {#if saved > 0}
      <button slot="end">Undo</button>
    {/if}
  </headerbar>
  <label>{saved} saves</label>
</window>
