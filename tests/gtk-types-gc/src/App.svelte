<script>
  import { StringList } from 'svelters:gtk/gtk';
  import { File } from 'svelters:gtk/gio';

  // One wrapper is kept for good. "make" fills a pool with 80 more; "drop" empties the pool, and the
  // garbage collector must then release their SHARED handles (the renderer's finalizer calls destroy).
  const kept = new StringList({ strings: ['keep'] });
  let pool = [];
  let made = $state(0);

  function make() {
    for (let i = 0; i < 40; i++) {
      const list = new StringList({ strings: ['x' + i] });
      list.append('y');
      pool.push(list, File.newForPath('/tmp/' + i));
      made++;
    }
  }
  function drop() {
    pool = [];
  }
  globalThis.__kept = kept;
</script>

<window title="gc">
  <box orientation="vertical">
    <button onclicked={make}>make {made}</button>
    <button onclicked={drop}>drop</button>
  </box>
</window>
