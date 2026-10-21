<script>
  // R2: `bind:` lowered from the observe facts (plans/milestones/R2-compiler-observe.md).
  //   entry A  bind:text (exact)              -> $n.bind_prop: native changes reach `name` through observation
  //            bind:cursor-position (read-only, exact) -> $n.bind_readonly, no annotation needed
  //   entry B  bind:text|nonreactive          -> $n.bind_once: read once, then write-only
  //            bind:cursor-position|nonreactive -> one snapshot after the first layout pass
  // "native" writes both entries behind the runtime's back (bind:this + setProp), as the toolkit would;
  // the script then announces the changes (`changed` steps). Only A follows them.
  let name = $state('Ann');
  let draft = $state('Ann');
  let pos = $state(-1);
  let pos0 = $state(-1);
  let a = $state();
  let b = $state();
</script>

<window title="bind observe">
  <box orientation="vertical" spacing="6">
    <entry bind:this={a} bind:text={name} bind:cursor-position={pos} />
    <entry bind:this={b} bind:text|nonreactive={draft} bind:cursor-position|nonreactive={pos0} />
    <button onclicked={() => { a.setProp('text', 'Zed'); b.setProp('text', 'Zed'); }}>native</button>
    <button onclicked={() => { name = 'Bob'; draft = 'Bob'; }}>state</button>
    <label>name {name}</label>
    <label>draft {draft}</label>
    <label>pos {pos}</label>
  </box>
</window>
