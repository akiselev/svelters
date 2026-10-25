<script>
  // Input-latency probes: each control's reaction is a label whose text changes, so a test can
  // count the frames from the input to the first frame that draws the reaction
  // (crates/svelters-egui/tests/input_latency.rs).
  let signal = $state(0);
  let dom = $state(0);
  let checked = $state(false);
  let text = $state('');
  const rows = Array.from({ length: 60 }, (_, i) => i);
</script>

<window title="Latency" default-width={480} default-height={520}>
  <box class="column">
    <menu-bar>
      <menu label="Alpha">
        <action label="Alpha item" />
      </menu>
      <menu label="Beta">
        <action label="Beta item" />
      </menu>
    </menu-bar>
    <box class="row">
      <button onclicked={() => signal++}>Signal</button>
      <label>signal {signal}</label>
    </box>
    <box class="row">
      <button onclick={() => dom++}>Dom</button>
      <label>dom {dom}</label>
    </box>
    <box class="row">
      <checkbox bind:checked>Check</checkbox>
      {#if checked}<label>checked on</label>{/if}
    </box>
    <box class="row">
      <text-edit hint-text="Field" flex-grow={1} bind:text />
      <label>typed [{text}]</label>
    </box>
    <scroll-area class="scroller">
      {#each rows as r}<label>row {r}</label>{/each}
    </scroll-area>
  </box>
</window>

<style>
  .column {
    gap: 6px;
  }
  .row {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }
  .scroller {
    height: 160px;
  }
</style>
