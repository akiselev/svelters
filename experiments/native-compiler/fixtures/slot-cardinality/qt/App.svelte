<!-- Qt-vocabulary copy of ../App.svelte (the qt compile target). -->
<!-- A window's main slot (GTK `child`, Qt `central`) takes one child: two are a compile-time error
     on both backends (slot_cardinality). Alternatives inside {#if} may share the slot. -->
<script>
  let wide = $state(false);
</script>

<main-window window-title="Cardinality">
  <label>first</label>
  <label>second</label>
</main-window>

<main-window window-title="Alternatives are fine">
  {#if wide}
    <label>wide</label>
  {:else}
    <label>narrow</label>
  {/if}
</main-window>
