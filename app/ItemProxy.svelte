<script>
    export let store;
    export let target;
</script>

<listViewProxy {target}>
    <slot item={$store} />
</listViewProxy>
