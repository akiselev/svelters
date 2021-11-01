<script lang="ts">
    import { onDestroy, onMount } from "svelte";

    import Button from "./Button.svelte";
    import Inner from "./Inner.svelte";
    let isOpen = true;
    let window: any;
    const onClick = () => {
        console.log("....");
        isOpen = !isOpen;
        return "WTF";
    };

    onMount(() => {
        window.$.show();
    });
    onDestroy(() => {
        window.$.destroy();
    });
</script>

<appWindow
    title="Test Tittle"
    decorated={true}
    hideOnClose={true}
    bind:this={window}
>
    <hBox>
        {#if isOpen}
            <Inner />
        {/if}
        <button id="btn5" on:clicked={onClick} label="Toggle " hAlign="end" />
    </hBox>
    <!-- <child id="titlebar">
        {#if titlebar}
            <button
                label="TITLEBAR"
                on:clicked={() => {
                    console.log("pew pew");
                    titlebar = false;
                }}
            />
        {/if}
    </child> -->
</appWindow>

<style>
    .btn {
        background-color: #444;
    }
    .btn label {
        color: #f00;
    }
</style>
