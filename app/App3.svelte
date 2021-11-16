<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import Inner from "./Inner.svelte";
    let isOpen = true;
    let window: any;
    let count = 0;
    const onClick = () => {
        console.log("....");
        count += 1;
        return "WTF";
    };

    onMount(() => {
        window.show();
    });
    onDestroy(() => {
        window.destroy();
    });
</script>

<appWindow title="Test Tittle" decorated={true} bind:this={window}>
    <button
        id="btn35"
        on:clicked={onClick}
        label="Toggle {count}"
        halign="end"
        canFocus={true}
        sensitive={true}
        focusable={true}
    />
</appWindow>

<style>
    .btn {
        background-color: #444;
    }
    .btn label {
        color: #f00;
    }
</style>
