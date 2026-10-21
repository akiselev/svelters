<!-- kinds: NSTextField NSSearchField NSTokenField NSComboBox NSTextView NSScrollView -->
<script>
  // `echo` labels count the change signal of a binding whose echo strategy is `none`: they must stay 0. The
  // combo box binding is `compare`: -selectItemAtIndex: posts NSComboBoxSelectionDidChangeNotification itself and
  // the host drops the echo by comparing, so its label counts notices instead.
  let name = $state('');
  let query = $state('');
  let flavor = $state(0);
  let notes = $state('');
  let echoName = $state(0);
  let flavorNotices = $state(0);
  let echoNotes = $state(0);
  $effect(() => {
    name = 'Ada';
    flavor = 1;
    notes = 'A text view in a scroll view.\nIt keeps its own frame.';
  });
</script>

<stack-view orientation="vertical" alignment="leading" spacing="8">
  <stack-view orientation="horizontal" spacing="8">
    <text-field placeholder-string="Name" bind:string-value={name} oncontrol-text-did-change={() => echoName++} />
    <text-field string-value={`hello ${name}`} editable={false} bezeled={false} draws-background={false} />
    <text-field string-value={`echo name: ${echoName}`} editable={false} bezeled={false} draws-background={false} />
  </stack-view>
  <stack-view orientation="horizontal" spacing="8">
    <search-field placeholder-string="Search" maximum-recents={5} sends-whole-search-string={true} onaction={() => (query = "searched")} />
    <text-field string-value={`query: ${query}`} editable={false} bezeled={false} draws-background={false} />
  </stack-view>
  <token-field string-value="alpha, beta, gamma" token-style="rounded" completion-delay={0.2} />
  <stack-view orientation="horizontal" spacing="8">
    <combo-box object-values={['Vanilla', 'Chocolate', 'Strawberry']} completes={true} number-of-visible-items={3} bind:index-of-selected-item={flavor} oncombo-box-selection-did-change={() => flavorNotices++} />
    <text-field string-value={`flavor ${flavor}`} editable={false} bezeled={false} draws-background={false} />
    <text-field string-value={`selection notices: ${flavorNotices}`} editable={false} bezeled={false} draws-background={false} />
  </stack-view>
  <scroll-view has-vertical-scroller={true} border-type="bezel-border">
    <text-view frame={{ x: 0, y: 0, width: 360, height: 120 }} rich-text={false} allows-undo={true} bind:string={notes} ontext-did-change={() => echoNotes++} />
  </scroll-view>
  <text-field string-value={`notes: ${notes.length} characters`} editable={false} bezeled={false} draws-background={false} />
  <text-field string-value={`echo notes: ${echoNotes}`} editable={false} bezeled={false} draws-background={false} />
</stack-view>
