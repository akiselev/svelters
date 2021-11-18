<script lang="ts">
    import { createEventDispatcher, afterUpdate } from "svelte";

    const dispatch = createEventDispatcher();
</script>

<button class="btn" label="Text" halign="start" valign="end" on:clicked />

<style>
    .btn {
        background-color: black;
    }
</style>
