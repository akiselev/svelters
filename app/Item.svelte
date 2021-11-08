<script lang="ts">
    import { createEventDispatcher, afterUpdate } from "svelte";

    export let data: any;

    const onClick = () => {
        console.log(">>> ", JSON.stringify(data, null, 2));
    };

    const dispatch = createEventDispatcher();
</script>

<button label={data?.text || "NONE"} on:clicked={onClick} hExpand={true} />

<style>
    .btn {
        background-color: black;
    }
</style>
