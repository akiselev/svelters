<script>
    export let text: string;
</script>

<qPushButton {text} on:clicked />
<slot />
<svelte:options namespace="foreign" />
