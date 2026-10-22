<script lang="ts">
  // RA1: <quantity-field> is not an egui built-in. The app crate defines it in Rust
  // (`#[svelters::widget]`, examples/egui-kinds/src/lib.rs); the compiler learns it from the
  // dumped spec fragment (.svelters/app/kinds.json) and svelte-check from kinds.d.ts.
  // EV2b: `pick` is a native behaviour (`#[svelters::behavior]`), a Svelte action from the dumped
  // svelters:behaviors/cad module (typed by behaviors.d.ts); it runs in Rust every frame.
  import type { Dim, PickRef } from 'svelters:app/cad';
  import { pick } from 'svelters:behaviors/cad';

  let width: number | null = $state(0.012);
  let dim: Dim = $state('length');
  let accepted = $state(0);
  let edits = $state(0);
  let picking = $state(true);
  let picked = $state('none');
  const accepts = ['mcad.face'];
</script>

<window title="Kinds" default-width={360} default-height={240}>
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
    <label use:pick={{ accepts, active: picking, onpicked: (r: PickRef) => (picked = `${r.kind} ${r.label}`) }}>Face</label>
    <label>picked {picked}</label>
    <button onclicked={() => (picking = !picking)}>Toggle pick</button>
    <box use:pick={{ accepts: ['mcad.body'] }}><label>Body</label></box>
  </box>
</window>

<style>
  .column {
    gap: 8px;
    padding: 8px;
  }
</style>
