<script>
    import Component from "./Component.svelte";

    export let destroy;

    let isOpen = false;

    const onClick = () => {
        isOpen = !isOpen;
    };
</script>

<button on:click={onClick}>Click me</button>
{#if isOpen}
    <Component />
{/if}
<button on:click={destroy}>Destroy all</button>

<style>
    :global(button) {
        margin: 16px;
        padding: 4px;
        border: 1px solid #333;
    }
</style>
