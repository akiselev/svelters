<script>
    import { createEventDispatcher } from "svelte";
    export let text;

    const dispatch = createEventDispatcher();
</script>

<button
    label={text}
    hAlign="start"
    vAlign="end"
    direction="rtl"
    on:click={() => dispatch("click")}
/>
<!-- <slot />
<svelte:options namespace="foreign" /> -->
