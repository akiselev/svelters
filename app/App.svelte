<script>
    import { onMount } from "svelte";

    import Button from "./Button.svelte";
    let count2: string = 0;
    let comp;
    const onClick = () => {
        count2 += 1;
    };

    onMount(() => {
        global.comp = comp;
    });
</script>

<!-- <Button text="Hello {count}" on:clicked={onClick} > -->
<Button text="Hello {count2}" on:clicked={onClick} bind:this={comp} />
<!-- </Button> -->
