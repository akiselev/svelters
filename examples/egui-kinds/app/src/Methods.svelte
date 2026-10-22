<script lang="ts">
  // RA3: the methods of a Rust-defined kind (`#[svelters::widget] impl QuantityField`), called on
  // the element `bind:this` gives (typed by kinds.d.ts: `QuantityFieldElement`).
  // examples/egui-kinds/tests/methods.rs loads this file from source.
  import type { QuantityFieldElement } from '../.svelters/app/kinds';

  let field: QuantityFieldElement | undefined = $state();
  let width: number | null = $state(0.012);
  let accepted = $state(0);
  let edits = $state(0);
  let result = $state('none');
  let depth: number | null = $state(0.002);
</script>

<window title="Methods" default-width={360} default-height={240}>
  <box class="column">
    <quantity-field
      bind:this={field}
      label="Width"
      dim="length"
      bind:value={width}
      onaccepted={(v) => (accepted = v)}
      onedited={() => edits++}
    />
    <label>width {width} accepted {accepted} edits {edits}</label>
    <label>result {result}</label>
    <quantity-field label="Depth" dim="length" bind:value|commit={depth} />
    <label>depth {depth}</label>
    <button onclicked={() => (result = `parses ${field?.setText('40 mm')}`)}>Set text</button>
    <button onclicked={() => (result = `bad ${field?.call('setText', 'forty')}`)}>Set bad</button>
    <button onclicked={() => (result = `committed ${field?.commit()}`)}>Commit</button>
    <button onclicked={() => (result = `cm ${field?.valueIn('cm')} in ${field?.valueIn('in', 2)}`)}>Read</button>
    <button onclicked={() => field?.focus()}>Focus</button>
    <button onclicked={() => (result = `moved ${field?.moveCaret({ line: 0, column: 2 })}`)}>Caret</button>
    <button
      onclicked={() => {
        try {
          field?.moveCaret({ line: 3, column: 0 });
        } catch (e) {
          result = `threw ${(e as { code?: string }).code}`;
        }
      }}>Caret out</button
    >
    <button
      onclicked={() => {
        try {
          (field as any).valueIn(3);
          result = 'no error';
        } catch (e) {
          result = `error ${(e as Error).message}`;
        }
      }}>Wrong</button
    >
  </box>
</window>

<style>
  .column {
    gap: 4px;
    padding: 8px;
  }
</style>
