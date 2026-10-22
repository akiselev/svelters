<!--
@case transition:fade on a native label: the outro runs to its end (outroend), then the element
goes; the intro brings it back (fixtures/motion-egui, crates/svelters-egui/tests/motion.rs).
-->
<script>
  import { fade } from 'svelte/transition';
  import { Window, Column, Button } from 'svelters:testkit';

  let shown = $state(true);
  let ended = $state(0);
</script>

<Window title="Transition">
  <Column>
    <Button text="Toggle" onclick={() => (shown = !shown)} />
    {#if shown}
      <label transition:fade={{ duration: 120 }} onoutroend={() => ended++}>Faded</label>
    {/if}
    <label>Outros ended: {ended}</label>
  </Column>
</Window>

<!--
@test
expect "Faded"
expect "Outros ended: 0"
click "Toggle"
wait 300
expect no "Faded"
expect "Outros ended: 1"
click "Toggle"
wait 300
expect "Faded"
snapshot "back"
-->
