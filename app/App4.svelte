<script>
    import { onDestroy } from "svelte";
    import Button from "./Button.svelte";

    let isOpen = true;
    let window;
    onDestroy(() => {
        console.log("Destroying window....");
        window.destroy();
        window = null;
    });
</script>

<appWindow
    title="Test Tittle"
    decorated={true}
    visible={true}
    bind:this={window}
>
    {#if isOpen}
        <Button
            on:clicked={() => {
                isOpen = false;
            }}
        />
    {/if}
</appWindow>

<style>
    .btn {
        background-color: black;
    }
</style>
