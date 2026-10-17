<!-- QMainWindow wrapper: snippet props fill the window's menubar/statusbar slots through
     <sv-slot>; without a `status` snippet a local default renders, routed by kind to the
     statusbar slot at the render site (the compiler sees the snippet's root element). -->
<script>
  let { menu, status, children } = $props();
</script>

{#snippet defaultStatus()}
  <statusbar />
{/snippet}

<window title="Main">
  {#if menu}
    <sv-slot name="menubar">{@render menu()}</sv-slot>
  {/if}
  {@render children()}
  {#if status}
    <sv-slot name="statusbar">{@render status()}</sv-slot>
  {:else}
    {@render defaultStatus()}
  {/if}
</window>
