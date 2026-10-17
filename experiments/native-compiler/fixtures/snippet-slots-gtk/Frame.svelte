<!-- A window wrapper: the `titlebar` snippet prop goes into the window's native titlebar slot
     through <sv-slot>; `children` fills the window's child. -->
<script>
  let { title, titlebar, children } = $props();
</script>

<window {title}>
  {#if titlebar}
    <sv-slot name="titlebar">{@render titlebar()}</sv-slot>
  {/if}
  {@render children()}
</window>
