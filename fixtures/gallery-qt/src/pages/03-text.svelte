<!-- kinds: QLineEdit QLabel QPlainTextEdit QTextEdit QTextDocument QTextBrowser QKeySequenceEdit -->
<script>
  // Text: line edit, label, plain/rich text edits, text browser, key sequence edit.
  let plain = $state('plain');
  let rich = $state('rich');
  let echoPlain = $state(0);
  let echoRich = $state(0);
  let returns = $state(0);
  let links = $state(0);
  let rich_edit = $state();
  let blocks = $state(0);
  $effect(() => {
    // programmatic writes after mount: must not come back as change events
    plain = 'plain text';
    rich = 'rich text';
    // The document behind a text edit is a borrowed handle of its own kind (text-document).
    const doc = rich_edit.call('document');
    if (doc) {
      doc.setProp('undo-redo-enabled', true);
      doc.call('setPlainText', 'one\ntwo');
      blocks = doc.getProp('block-count');
    }
  });
</script>

<box-layout direction="top-to-bottom" spacing="4">
  <line-edit text="Password" echo-mode="password" clear-button-enabled={true} max-length={20} onreturn-pressed={() => returns++} />
  <label text="return-pressed: {returns}" />
  <label text="<a href='x'>link</a> word wrap" text-format="rich-text" word-wrap={true} margin={2} onlink-activated={() => links++} />
  <plain-text-edit bind:plain-text={plain} placeholder-text="Plain" line-wrap-mode="no-wrap" maximum-height={50} ontext-changed={() => echoPlain++} />
  <label text="echo plain-text: {echoPlain}" />
  <text-edit bind:plain-text={rich} accept-rich-text={true} placeholder-text="Rich" maximum-height={50} ontext-changed={() => echoRich++} />
  <label text="echo plain-text (rich): {echoRich}" />
  <label text="document blocks: {blocks}" />
  <text-browser bind:this={rich_edit} open-links={false} maximum-height={40} />
  <key-sequence-edit key-sequence="Ctrl+K" clear-button-enabled={true} />
</box-layout>
