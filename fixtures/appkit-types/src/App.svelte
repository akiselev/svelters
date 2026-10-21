<script>
  // AppKit twin of fixtures/gtk-types and fixtures/qt-types (plans/research/NON-GUI-TYPES.md section 7, N4).
  // On the real host (examples/counter-mac/tests/counter.rs) every value below is native; the op-log hosts
  // answer the constructor-shaped calls (construct, `init…:`) with handles and everything else with null.
  import { NSColor, NSFont } from 'svelters:appkit/appkit';
  import { NSURL, NSUndoManager, NSItemProvider } from 'svelters:appkit/foundation';

  // A value from a string (`new NSURL(s)` is `-[NSURL initWithString:]`), with methods.
  const home = new NSURL('https://example.com/docs/index.html');
  // An object made with construct-time properties (alloc/init, then KVC): SHARED, owned by JS's reference.
  const undo = new NSUndoManager({ levelsOfUndo: 5 });
  // Values from class factories, passed to element props.
  const color = NSColor.colorWithSRGBRedGreenBlueAlpha(0.8, 0.1, 0.1, 1);
  const font = NSFont.boldSystemFontOfSize(15);
  // A completion-handler method is a Promise (the block completes on a background queue; the core hops
  // to the main thread before converting the NSData).
  const provider = new NSItemProvider();
  provider.registerObjectVisibility('from a provider', 0);
  let loaded = $state('loading');
  provider.loadDataRepresentationForTypeIdentifier('public.utf8-plain-text').then(
    (bytes) => (loaded = bytes ? String.fromCharCode(...bytes) : 'none'),
    (e) => (loaded = `error ${e.code}`),
  );

  // Dropped wrappers release their handle when the engine collects them, which the Node and QuickJS
  // hosts do at different moments: the op log stays comparable only while the app holds what it made.
  globalThis.__fixtureKeep = [home, undo, provider];
</script>

<window title="Types">
  <stack-view orientation="vertical" spacing="6">
    <text-field string-value={home.host} text-color={color} font={font} editable={false} />
    <text-field string-value={`undo levels ${undo.levelsOfUndo}`} editable={false} />
    <text-field string-value={loaded} editable={false} />
  </stack-view>
</window>
