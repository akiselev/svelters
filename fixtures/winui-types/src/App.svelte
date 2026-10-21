<script>
  // WinUI non-GUI types from Svelte (N4, plans/research/NON-GUI-TYPES.md section 7): Windows Runtime classes as ES
  // module named exports. Run on the real host by `counter-win --fixture winui-types --selftest` (the windows-winui
  // CI job); the JS-only tests just build it (the Node TestHost has no WinRT).
  import { Uri } from 'svelters:winui/windows.foundation';
  import { PropertySet } from 'svelters:winui/windows.foundation.collections';
  import { Colors } from 'svelters:winui/microsoft.ui';
  import { SolidColorBrush } from 'svelters:winui/microsoft.ui.xaml.media';
  import { DataPackage } from 'svelters:winui/windows.applicationmodel.datatransfer';

  // A runtime class made through its activation factory (`new` picks the factory method by argument count).
  const uri = new Uri('https://example.com:8080/docs?q=1#top');

  // A model: an observable map, one JS object per native object, its events through a dynamic delegate.
  const set = new PropertySet();
  let size = $state(0);
  let changes = $state(0);
  set.on('map-changed', () => {
    changes++;
    size = set.size;
  });
  set.insert('a', 1);
  set.insert('b', 'two');

  // A DependencyObject (dispatcher thread class) made with construct-time props; `color` is a DependencyProperty,
  // so the template re-reads it when it changes natively (no `$state` mirror).
  const brush = new SolidColorBrush({ color: Colors.red, opacity: 0.5 });

  // An async member: a Promise resolved after the completion hopped to the UI thread.
  const pkg = new DataPackage();
  pkg.setText('copied');
  let pasted = $state('pending');
  pkg.getView().getTextAsync().then(
    (t) => (pasted = t),
    (e) => (pasted = `error: ${e.code}`),
  );

  function add() {
    set.insert('c', true);
    brush.color = Colors.blue;
  }

  // The app keeps what it made (dropped wrappers are released when the engine collects them).
  globalThis.__fixtureKeep = [uri, set, brush, pkg];
</script>

<window title="WinUI types">
  <stack-panel orientation="vertical" spacing="8" padding="16">
    <text-block>{`${uri.host}:${uri.port}${uri.path}`}</text-block>
    <text-block>{`size ${size}, changes ${changes}`}</text-block>
    <text-block>{`brush ${brush.color.r},${brush.color.g},${brush.color.b} @ ${brush.opacity}`}</text-block>
    <text-block>{`pasted ${pasted}`}</text-block>
    <button onclick={add}>Add</button>
  </stack-panel>
</window>
