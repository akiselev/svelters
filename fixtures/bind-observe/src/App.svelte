<script>
  // R2: `bind:` lowered from the observe facts (plans/milestones/R2-compiler-observe.md).
  //   entry A  bind:text (exact)              -> $n.bind_prop: native changes reach `name` through observation
  //            bind:cursor-position (read-only, exact) -> $n.bind_readonly, no annotation needed
  //   entry B  bind:text|nonreactive          -> $n.bind_once: read once, then write-only
  //            bind:cursor-position|nonreactive -> one snapshot after the first layout pass
  // "native" writes both entries through bind:this + setProp (the host reads the value back and drops
  // its echo, so no state changes) and selects a region of A, which moves its cursor natively (the real
  // toolkits announce `cursor-position`). The script then changes both texts natively to a different
  // value (value-carrying `changed` steps); only A follows.
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
    <button onclicked={() => { a.setProp('text', 'Zed'); b.setProp('text', 'Zed'); a.call('selectRegion', 1, 2); }}>native</button>
    <button onclicked={() => { name = 'Bob'; draft = 'Bob'; }}>state</button>
    <label>name {name}</label>
    <label>draft {draft}</label>
    <label>pos {pos}</label>
  </box>
</window>
