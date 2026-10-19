<script>
  import { TextBuffer, StringList } from 'svelters:gtk/gtk';
  import { File, FileIcon } from 'svelters:gtk/gio';

  // One native TextBuffer shared by two views; its `changed` signal drives a counter.
  const buffer = new TextBuffer({ text: 'Hello' });
  let length = $state(5);
  $effect(() => buffer.on('changed', (text) => (length = text.length)));

  // A native model built and extended from JS.
  const names = new StringList({ strings: ['one', 'two'] });
  names.append('three');

  // An icon from a path: strings keep meaning a theme name, files are explicit.
  const file = File.newForPath('/usr/share/pixmaps/svelters.svg');
  const icon = new FileIcon({ file });

  // Dropped wrappers release their handle when the engine collects them, which the Node and QuickJS
  // hosts do at different moments: the op log stays comparable only while the app holds what it made.
  globalThis.__fixtureKeep = [names, file];
</script>

<window title="Types">
  <box orientation="vertical" spacing={6}>
    <text-view buffer={buffer} />
    <text-view buffer={buffer} monospace />
    <label label={`${length} characters`} />
    <image gicon={icon} pixel-size={16} />
  </box>
</window>
