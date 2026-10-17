<!-- kinds: QLineEdit QLabel QPlainTextEdit QTextEdit QTextBrowser QKeySequenceEdit -->
<script>
  // Text: line edit, label, plain/rich text edits, text browser, key sequence edit.
  let plain = $state('plain');
  let rich = $state('rich');
  let echoPlain = $state(0);
  let echoRich = $state(0);
  let returns = $state(0);
  let links = $state(0);
  $effect(() => {
    // programmatic writes after mount: must not come back as change events
    plain = 'plain text';
    rich = 'rich text';
  });
</script>

<box orientation="vertical" spacing="4">
  <entry text="Password" echoMode="password" clearButtonEnabled={true} maxLength={20} onreturnPressed={() => returns++} />
  <label label="returnPressed: {returns}" />
  <label label="<a href='x'>link</a> word wrap" textFormat="rich-text" wordWrap={true} margin={2} onlinkActivated={() => links++} />
  <plaintextedit bind:plainText={plain} placeholderText="Plain" lineWrapMode="no-wrap" maximumHeight={50} onchanged={() => echoPlain++} />
  <label label="echo plainText: {echoPlain}" />
  <textedit bind:plainText={rich} acceptRichText={true} placeholderText="Rich" maximumHeight={50} onchanged={() => echoRich++} />
  <label label="echo plainText (rich): {echoRich}" />
  <textbrowser openLinks={false} maximumHeight={40} />
  <keysequenceedit keySequence="Ctrl+K" clearButtonEnabled={true} />
</box>
