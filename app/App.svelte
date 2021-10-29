<script lang="ts">
    import { onMount } from "svelte";

    import Button from "./Button.svelte";
    let count2: number = 0;
    let titlebar = true;
    let btn2 = null;
    let window: any;
    const onClick = () => {
        console.log("....");
        count2 += 1;
        console.log("Count ", count2);
        return "WTF";
    };

    const onClick2 = () => {
        console.log("....");
        count2 -= 1;
        console.log("Count ", count2);
        return "WTF";
    };
    onMount(() => {
        window.instance.show();
    });
</script>

<appWindow
    title="Test Tittle"
    decorated={true}
    hideOnClose={true}
    bind:this={window}
>
    <hBox>
        {#if count2 == 0}
            <Button
                text="Hello {count2}"
                on:clicked={onClick}
                bind:this={btn2}
            />
        {/if}
        {#if count2 != 0}
            <button
                vAlign="fill"
                id="btn3"
                label="Pew pew"
                on:clicked={onClick2}
            />
        {/if}
        <button vAlign="fill" id="btn2" label="Pew pew" class="btn" />
    </hBox>
    <child id="titlebar">
        {#if titlebar}
            <button
                label="TITLEBAR"
                on:clicked={() => {
                    console.log("pew pew");
                    titlebar = false;
                }}
            />
        {/if}
    </child>
</appWindow>

<style>
    .btn {
        background-color: #444;
    }
    :global(.btn label) {
        color: #f00;
    }
</style>
