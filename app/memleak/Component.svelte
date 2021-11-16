<script>
    let count = 0;
    const onClick = () => {
        count = count + 1;
    };
</script>

<button on:click={onClick}>Clicked {count} times</button>
