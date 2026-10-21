<script>
  // Qt twin of fixtures/bind-observe (R2, plans/milestones/R2-compiler-observe.md).
  //   line-edit A  bind:text (exact, NOTIFY textChanged)  -> $n.bind_prop
  //   line-edit B  bind:text|nonreactive                  -> $n.bind_once
  // The Qt read-only binding (`$n.bind_readonly`) is in fixtures/bind-readonly-qt: no Qt element has a
  // read-only prop the op-log hosts' Qt schema knows.
  let name = $state('Ann');
  let draft = $state('Ann');
  let a = $state();
  let b = $state();
</script>

<main-window window-title="bind observe">
  <box-layout direction="top-to-bottom" spacing="6">
    <line-edit bind:this={a} bind:text={name} />
    <line-edit bind:this={b} bind:text|nonreactive={draft} />
    <push-button onclicked={() => { a.setProp('text', 'Zed'); b.setProp('text', 'Zed'); }}>native</push-button>
    <push-button onclicked={() => { name = 'Bob'; draft = 'Bob'; }}>state</push-button>
    <label>name {name}</label>
    <label>draft {draft}</label>
    <label>size n/a</label>
  </box-layout>
</main-window>
