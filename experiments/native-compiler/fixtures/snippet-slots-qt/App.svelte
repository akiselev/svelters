<!-- Snippets as named-slot content on Qt: an inline {#snippet menu()} prop and the implicit
     children go into QMainWindow slots of the wrapper component; a status snippet prop that
     comes and goes swaps the wrapper's default statusbar. GTK's window has no menubar slot. -->
<script>
  import MainWindow from './MainWindow.svelte';

  let n = $state(0);
  let own = $state(false);
</script>

{#snippet ownStatus()}
  <status-bar />
{/snippet}

<MainWindow status={own ? ownStatus : undefined}>
  {#snippet menu()}
    <menu-bar />
  {/snippet}
  <box-layout direction="top-to-bottom">
    <label>Clicked {n} times</label>
    <push-button onclicked={() => n++}>Click</push-button>
    <push-button onclicked={() => (own = !own)}>Status</push-button>
  </box-layout>
</MainWindow>
