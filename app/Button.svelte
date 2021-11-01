<script lang="ts">
    import { createEventDispatcher, afterUpdate } from "svelte";
    export let text;
    export let id;

    afterUpdate(() => {
        console.log("New text: " + text);
    });
    const dispatch = createEventDispatcher();
</script>

<button class="btn" label={text} {id} hAlign="start" vAlign="end" on:clicked />

<style>
    .btn {
        background-color: black;
    }
</style>
