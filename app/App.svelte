<script lang="ts">
    import { onMount } from "svelte";

    import Button from "./Button.svelte";
    let count2: number = 0;
    let titlebar = true;
    let window: any;
    const onClick = () => {
        console.log("....");
        count2 += 1;
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
            <Button text="Hello {count2}" on:clicked={onClick} />
        {/if}
        {#if count2 != 0}
            <button
                vAlign="fill"
                name="btn3"
                label="Pew pew"
                on:clicked={() => {
                    count2 -= 1;
                }}
            />
        {/if}
        <button vAlign="fill" name="btn2" label="Pew pew" />
    </hBox>
    <child name="titlebar">
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
<!-- <Button text="Hello {count}" on:clicked={onClick} > -->
<!-- </Button> -->
