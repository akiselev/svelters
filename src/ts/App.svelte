<script>
import { onMount } from "svelte";

    import Button from "./Button.svelte";
    let count = 0;
    let comp;
    const onClick = () => {
        count += 1;
    }

    onMount(() => {
        global.comp = comp;
    });
</script>

<!-- <Button text="Hello {count}" on:clicked={onClick} > -->
    <Button text="Hello {count}" on:clicked={onClick} bind:this={comp} />
<!-- </Button> -->


