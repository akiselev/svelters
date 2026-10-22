<script lang="ts">
  // RA1: <quantity-field> is not an egui built-in. The app crate defines it in Rust
  // (`#[svelters::widget]`, examples/egui-kinds/src/lib.rs); the compiler learns it from the
  // dumped spec fragment (.svelters/app/kinds.json) and svelte-check from kinds.d.ts.
  import type { Dim } from 'svelters:app/cad';

  let width: number | null = $state(0.012);
  let dim: Dim = $state('length');
  let accepted = $state(0);
  let edits = $state(0);
</script>

<window title="Kinds" default-width={360} default-height={200}>
  <box class="column">
    <quantity-field
      label="Width"
      {dim}
      bind:value={width}
      problem={width !== null && width > 1 ? 'wider than a metre' : null}
      onaccepted={(v) => (accepted = v)}
      onedited={() => edits++}
    />
    <label>width {width} accepted {accepted}</label>
    <label>edits {edits > 0 ? 'some' : 'none'}</label>
  </box>
</window>

<style>
  .column {
    gap: 8px;
    padding: 8px;
  }
</style>
