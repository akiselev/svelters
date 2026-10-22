<script lang="ts">
  // RA3: the fn form (<level-meter>, <step-count>), wrap_widget! (<wrapped-drag-value>) and a container
  // kind (<collapsing-section>), all defined in examples/egui-kinds/src/lib.rs.
  // examples/egui-kinds/tests/widgets.rs loads this file from source.
  let level = $state(0.25);
  let count = $state(2);
  let x = $state(1.5);
  let open = $state(true);
  let samples = $state([1, 3, 2, 5, 4]);
</script>

<window title="Widgets" default-width={360} default-height={320}>
  <box class="column">
    <level-meter {level} label="Load" />
    <step-count bind:count />
    <wrapped-drag-value bind:value={x} speed={0.5} suffix=" mm" />
    <collapsing-section title="Advanced" bind:open>
      <label>inside one</label>
      <label>inside two</label>
    </collapsing-section>
    <spark-line values={samples} />
    <label>count {count} x {x} open {open}</label>
    <button onclicked={() => (level = 0.75)}>More load</button>
    <button onclicked={() => (x = 4)}>Set x</button>
    <button onclicked={() => (samples = [...samples, 6])}>Sample</button>
  </box>
</window>

<style>
  .column {
    gap: 4px;
    padding: 8px;
  }
</style>
