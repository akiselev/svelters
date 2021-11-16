<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import Inner from "./Inner.svelte";
    let isOpen = true;
    let window: any;
    let hasFocus: boolean;
    const onClick = () => {
        console.log("....");
        isOpen = !isOpen;
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
    <vBox>
        <hBox>
            {#if isOpen}
                <Inner />
            {/if}
            <button
                id="btn35"
                on:clicked={onClick}
                label="Toggle {hasFocus ? 'World' : ''}"
                halign="end"
                canFocus={true}
                sensitive={true}
                focusable={true}
            />
            {#if isOpen}
                <button
                    id="btn5"
                    label="Toggle 2"
                    halign="end"
                    canFocus={true}
                    sensitive={true}
                    focusable={true}
                />
            {/if}
        </hBox>
    </vBox>
</appWindow>

<style>
    .btn {
        background-color: #444;
    }
    .btn label {
        color: #f00;
    }
</style>
