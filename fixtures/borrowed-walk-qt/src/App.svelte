<script>
  // Borrowed handles through bind:this: getters return renderer nodes. Objects we created come
  // back as their own bound nodes (identity); native-made ones (the scrolled window's internal
  // viewport) come back as one stable borrowed node that goes stale when its owner dies.
  // GTK and Qt name their getters differently, so each lookup tries both.
  const PARENT = ['getParent', 'parentWidget'];
  const ROOT = ['getRoot', 'window'];
  const INNER = ['getChild', 'viewport'];

  function native(node, names) {
    if (!node) return null;
    let last;
    for (const name of names) {
      try {
        return node.call(name);
      } catch (e) {
        if (!/unknown/i.test(String(e?.message))) throw e;
        last = e;
      }
    }
    throw last;
  }

  let win = $state();
  let box = $state();
  let entry = $state();
  let scroll = $state();
  let show = $state(true);
  let kept;
  let same = $state('(none)');
  let root = $state('(none)');
  let depth = $state('(none)');
  let inner = $state('(none)');
  let stale = $state('(none)');

  function walk() {
    const a = native(entry, PARENT);
    same = `same: ${a === native(entry, PARENT) && a === box}`;
    root = `root: ${native(entry, ROOT) === win}`;
    let n = 0;
    for (let cur = entry; ; n++) {
      cur = native(cur, PARENT);
      if (!cur) break;
    }
    depth = `depth: ${n}`;
    const v = native(scroll, INNER);
    kept = v;
    inner = `inner: ${v !== scroll && v === native(scroll, INNER) && native(v, PARENT) === scroll && v?.borrowed === true}`;
  }

  function drop() {
    show = false;
  }

  function probe() {
    try {
      native(kept, PARENT);
      stale = 'stale: alive';
    } catch (e) {
      stale = `stale: ${e.name}: ${e.reason}`;
    }
  }
</script>

<main-window window-title="borrowed-walk" bind:this={win}>
  <box-layout direction="top-to-bottom" spacing="6" bind:this={box}>
    <line-edit bind:this={entry} text="seed" />
    <push-button onclicked={walk}>Walk</push-button>
    <push-button onclicked={drop}>Drop</push-button>
    <push-button onclicked={probe}>Probe</push-button>
    <label>{same}</label>
    <label>{root}</label>
    <label>{depth}</label>
    <label>{inner}</label>
    <label>{stale}</label>
    {#if show}
      <scroll-area bind:this={scroll}>
        <box-layout direction="top-to-bottom" spacing="2">
          <label>row</label>
        </box-layout>
      </scroll-area>
    {/if}
  </box-layout>
</main-window>
