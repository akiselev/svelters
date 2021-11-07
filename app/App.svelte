<script lang="ts">
    import { onDestroy, onMount } from "svelte";

    import Button from "./Button.svelte";
    import Inner from "./Inner.svelte";
    import ListView from "./ListView.svelte";
    let isOpen = true;
    let window: any;
    let hasFocus: boolean;
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
    <vBox>
        <hBox>
            {#if isOpen}
                <Inner />
            {/if}
            <button
                id="btn5"
                bind:hasFocus
                on:clicked={onClick}
                label="Toggle {hasFocus ? 'World' : ''}"
                hAlign="end"
                canFocus={true}
                sensitive={true}
                focusable={true}
            >
                <controller:motion
                    on:motion={(event) =>
                        console.log(
                            event.details[0].containsPointer,
                            JSON.stringify(event, null, 2),
                        )}
                />
            </button>
        </hBox>
        <scrolled maxContentHeight={600}>
            <viewport>
                <ListView />
            </viewport>
        </scrolled>
    </vBox>
    <controller:keyboard
        on:key-pressed={(event) =>
            console.log(event.details[0], JSON.stringify(event, null, 2))}
    />
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
