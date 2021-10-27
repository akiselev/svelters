<script lang="ts">
    import { onMount } from "svelte";

    import Button from "./Button.svelte";
    let count2: number = 0;
    let comp;
    const onClick = () => {
        console.log("....");
        count2 += 1;
        console.log("Count ", count2);
        return "WTF";
    };

    onMount(() => {
        window.comp = comp;
    });
</script>

<!-- <Button text="Hello {count}" on:clicked={onClick} > -->
{#if count2 == 0}
    <Button text="Hello {count2}" on:clicked={onClick} bind:this={comp} />
{/if}
<!-- </Button> -->
