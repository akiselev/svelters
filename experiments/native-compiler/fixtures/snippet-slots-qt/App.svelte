<!-- Snippets as named-slot content on Qt: an inline {#snippet menu()} prop and the implicit
     children go into QMainWindow slots of the wrapper component; a status snippet prop that
     comes and goes swaps the wrapper's default statusbar. GTK's window has no menubar slot. -->
<script>
  import MainWindow from './MainWindow.svelte';

  let n = $state(0);
  let own = $state(false);
</script>

{#snippet ownStatus()}
  <statusbar />
{/snippet}

<MainWindow status={own ? ownStatus : undefined}>
  {#snippet menu()}
    <menubar />
  {/snippet}
  <box orientation="vertical">
    <label>Clicked {n} times</label>
    <button onclick={() => n++}>Click</button>
    <button onclick={() => (own = !own)}>Status</button>
  </box>
</MainWindow>
