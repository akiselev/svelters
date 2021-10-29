<script lang="ts">
    import { createEventDispatcher, afterUpdate } from "svelte";
    export let text;

    afterUpdate(() => {
        console.log("New text: " + text);
    });
    const dispatch = createEventDispatcher();
</script>

<button
    class="btn"
    label={text}
    id="btn"
    hAlign="start"
    vAlign="end"
    on:clicked
/>

<style>
    .btn {
        background-color: black;
    }
</style>
