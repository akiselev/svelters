<script>
  // Qt twin of fixtures/gtk-types (plans/research/NON-GUI-TYPES.md section 7, N2).
  import { QTextDocument, QIcon } from 'svelters:qt/gui';
  import { QStringListModel, QUrl } from 'svelters:qt/core';

  // One native QTextDocument shared by two editors. Made by JS, it is SHARED: JS's reference owns it
  // (no QObject parent). `doc.plainText` is reactive (R1: re-read after `contentsChanged`), so the label
  // needs no `$state` mirror.
  const doc = new QTextDocument({ plainText: 'Hello' });

  // A native model built and extended from JS (overloads dispatch at run time: index(row, column)).
  const names = new QStringListModel();
  names.setStringList(['one', 'two']);
  names.insertRows(2, 1);
  names.setData(names.index(2, 0), 'three');

  // An icon from a path: strings keep meaning a theme name, files are explicit (a QIcon value).
  const icon = new QIcon('/tmp/svelters-qt-types.png');
  // A value with methods.
  const home = new QUrl('https://example.com/docs/index.html');

  // Dropped wrappers release their handle when the engine collects them, which the Node and QuickJS
  // hosts do at different moments: the op log stays comparable only while the app holds what it made.
  globalThis.__fixtureKeep = [names, home];
</script>

<main-window window-title="Types">
  <box-layout direction="top-to-bottom" spacing="6">
    <text-edit document={doc} />
    <text-edit document={doc} />
    <label text={`${(doc.plainText ?? '').length} characters`} />
    <push-button icon={icon} text={home.host()} />
  </box-layout>
</main-window>
