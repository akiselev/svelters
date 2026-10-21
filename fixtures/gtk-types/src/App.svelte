<script>
  import { TextBuffer, StringList } from 'svelters:gtk/gtk';
  import { File, FileIcon } from 'svelters:gtk/gio';

  // One native TextBuffer shared by two views. `buffer.text` is reactive (R1): the label re-renders when
  // the text changes natively (GTK notify::text, delivered by the host), with no `$state` mirror.
  const buffer = new TextBuffer({ text: 'Hello' });

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
    <label label={`${buffer.text.length} characters`} />
    <image gicon={icon} pixel-size={16} />
  </box>
</window>
