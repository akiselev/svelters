<script lang="ts">
    import { onDestroy, onMount } from "svelte";

    import Button from "./Button.svelte";
    import Inner from "./Inner.svelte";
    import ListView from "./ListView.svelte";
    let isOpen = true;
    let isOpen2 = false;
    let window: any;
    let hasFocus: boolean;
    const onClick = () => {
        console.log("....");
        isOpen = !isOpen;
        return "WTF";
    };
    const onClick2 = () => {
        console.log("....");
        isOpen2 = !isOpen2;
        return "WTF";
    };

    onMount(() => {
        window.show();
    });
    onDestroy(() => {
        console.log("Destroying window....");
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
                hAlign="end"
                canFocus={true}
                sensitive={true}
                focusable={true}
            >
                <!-- <controller:motion on:motion={(event) => {}} /> -->
            </button>
            <button
                id="btn5"
                on:clicked={onClick2}
                label="Toggle 2"
                hAlign="end"
                canFocus={true}
                sensitive={true}
                focusable={true}
            >
                <!-- <controller:motion on:motion={(event) => {}} /> -->
            </button>
        </hBox>
        <ListView let:item>
            <button
                label={item.text}
                on:clicked={() => console.log(item.text)}
            />
        </ListView>
    </vBox>
    <!-- <controller:keyboard
        on:key-pressed={(event) =>
            console.log(event.details[0], JSON.stringify(event, null, 2))}
    /> -->
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
