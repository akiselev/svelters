<script lang="ts">
    import Button from "./Button.svelte";
    let count2: number = 0;
    let titlebar = true;
    let window: any;
    const onClick = () => {
        console.log("....");
        count2 = count2 + 1;
        console.log("Count ", count2);
        return "WTF";
    };
</script>

<button
    vAlign="fill"
    id="btn1"
    label="Pew pew"
    class="btn"
    on:clicked={onClick}
/>

<!-- <button
    vAlign="fill"
    id="btn3"
    label={hasFocus ? "Pew pew" : "_"}
    bind:hasFocus
    on:clicked={onClick2}
/>
{#if count2 % 2 === 0}
    <Button id="btn4" text="Hello2222 {count2}" />
{/if} -->
<style>
    .btn {
        background-color: #444;
    }
    .btn label {
        color: #f00;
    }
</style>
