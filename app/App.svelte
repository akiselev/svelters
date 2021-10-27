<script lang="ts">
    import { onMount } from "svelte";

    import Button from "./Button.svelte";
    let count2: string = 0;
    let comp;
    const onClick = () => {
        console.log("....");
        count2 += 1;
        console.log("Count ", count2);
        return "WTF";
    };

    onMount(() => {
        // global.comp = comp;
    });
</script>

<!-- <Button text="Hello {count}" on:clicked={onClick} > -->
<Button text="Hello {count2}" on:click={onClick} bind:this={comp} />
<!-- </Button> -->
