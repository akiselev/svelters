<script lang="ts">
  // RA3: an object-handle prop (`#[prop] doc: Option<Obj<DocHandle>>`), as K1's `<code-editor doc>`.
  // examples/egui-kinds/tests/methods.rs loads this file from source.
  import { DocHandle } from 'svelters:app/cad';
  import type { DocTitleElement } from '../.svelters/app/kinds';

  const plan = new DocHandle('plan.md');
  const notes = new DocHandle('notes.md');
  let doc: DocHandle | null = $state(plan);
  let title: DocTitleElement | undefined = $state();
  let result = $state('none');
</script>

<window title="Doc" default-width={300} default-height={200}>
  <box class="column">
    <doc-title bind:this={title} {doc} />
    <label>result {result}</label>
    <button onclicked={() => (doc = doc === plan ? notes : plan)}>Swap</button>
    <button onclicked={() => (doc = null)}>Clear</button>
    <button onclicked={() => (result = `name ${title?.docName()} edits ${plan.edits}`)}>Ask</button>
  </box>
</window>

<style>
  .column {
    gap: 4px;
    padding: 8px;
  }
</style>
