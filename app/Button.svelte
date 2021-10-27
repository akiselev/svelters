<script lang="ts">
    import { createEventDispatcher, afterUpdate } from "svelte";
    export let text;

    afterUpdate(() => {
        console.log("New text: " + text);
    });
    const dispatch = createEventDispatcher();
</script>

<button
    label={text}
    hAlign="start"
    vAlign="end"
    on:clicked={() => dispatch("clicked")}
/>
<!-- <slot />
<svelte:options namespace="foreign" /> -->
