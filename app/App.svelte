<script lang="ts">
    import { onDestroy, onMount } from "svelte";

    import Button from "./Button.svelte";
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

        const x = new JsValue({ test: "HELLO WORLD" });
        console.log(">>>><<<<", x.value.test);
    });
    onDestroy(() => {
        window.destroy();
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
        <button
            id="btn5"
            on:clicked={onClick}
            label="Toggle {hasFocus ? 'World' : ''}"
            hAlign="end"
            canFocus={true}
            sensitive={true}
            focusable={true}
        />
    </hBox>
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
