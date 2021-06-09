<script>
    export let text;
</script>

<qPushButton {text} on:clicked />
<slot />
<svelte:options namespace="foreign" />
