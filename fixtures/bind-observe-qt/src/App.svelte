<script>
  // Qt twin of fixtures/bind-observe (R2, plans/milestones/R2-compiler-observe.md).
  //   line-edit A  bind:text (exact, NOTIFY textChanged)  -> $n.bind_prop
  //   line-edit B  bind:text|nonreactive                  -> $n.bind_once
  //   box-layout   bind:size (read-only, exact through the Resize event) -> $n.bind_readonly; until the
  //                Qt backend observes events (R3) the host answers that it cannot observe it, and the
  //                binding keeps the values read at creation and after the first layout pass
  let name = $state('Ann');
  let draft = $state('Ann');
  let size = $state(null);
  let a = $state();
  let b = $state();
</script>

<main-window window-title="bind observe">
  <box-layout direction="top-to-bottom" spacing="6" bind:size>
    <line-edit bind:this={a} bind:text={name} />
    <line-edit bind:this={b} bind:text|nonreactive={draft} />
    <push-button onclicked={() => { a.setProp('text', 'Zed'); b.setProp('text', 'Zed'); }}>native</push-button>
    <push-button onclicked={() => { name = 'Bob'; draft = 'Bob'; }}>state</push-button>
    <label>name {name}</label>
    <label>draft {draft}</label>
    <label>size {size ? `${size.width}x${size.height}` : 'none'}</label>
  </box-layout>
</main-window>
