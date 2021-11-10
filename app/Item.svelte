<script lang="ts">
    import { createEventDispatcher, afterUpdate } from "svelte";

    export let data: any;

    const onClick = () => {
        console.log(">>> ", JSON.stringify(data, null, 2));
    };

    const dispatch = createEventDispatcher();
</script>

{#if data}
    <button label={data.text} on:clicked={onClick} hExpand={true} />
{/if}

<style>
    .btn {
        background-color: black;
    }
</style>
