<!-- Named native child slots, validated at compile time against the backend spec.
     GTK: window.titlebar takes a headerbar; headerbar has start/end slots. Qt has no headerbar. -->
<script>
  let saved = $state(0);
</script>

<window title="Slots">
  <header-bar slot="titlebar">
    <button slot="start" onclicked={() => saved++}>Save</button>
    {#if saved > 0}
      <button slot="end">Undo</button>
    {/if}
  </header-bar>
  <label>{saved} saves</label>
</window>
