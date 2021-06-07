<script>
    let count = 0;
    const onClick = () => {
        count += 1;
    }
</script>

<qPushButton text="Hello {count}" on:clicked={onClick} />
<svelte:options namespace="foreign" />
