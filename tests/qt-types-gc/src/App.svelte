<script>
  // Garbage collection of the Qt type layer's handles on the real host (examples/counter-qt/tests/host_types.rs).
  import { QStringListModel, QUrl, QMimeDatabase } from 'svelters:qt/core';

  // One wrapper is kept for good. "make" fills a pool with 40 SHARED models, 40 VALUE urls and 40 unique
  // databases; "drop" empties it, and the collector must then release every handle (models deleted: no parent).
  const kept = new QStringListModel();
  let pool = [];
  let made = $state(0);

  function make() {
    for (let i = 0; i < 40; i++) {
      const m = new QStringListModel();
      m.setStringList(['x' + i]);
      pool.push(m, new QUrl('https://example.com/' + i), new QMimeDatabase());
      made++;
    }
  }
  function drop() {
    pool = [];
  }
  globalThis.__kept = kept;
</script>

<main-window window-title="gc">
  <box-layout direction="top-to-bottom">
    <push-button onclicked={make}>make {made}</push-button>
    <push-button onclicked={drop}>drop</push-button>
  </box-layout>
</main-window>
