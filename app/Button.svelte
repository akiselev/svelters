<script lang="ts">
    import { createEventDispatcher, afterUpdate } from "svelte";
    export let text;

    afterUpdate(() => {
        console.log("New text: " + text);
    });
    const dispatch = createEventDispatcher();
</script>

<button label={text} name="btn" hAlign="start" vAlign="end" on:clicked />
<!-- <slot />
<svelte:options namespace="foreign" /> -->
