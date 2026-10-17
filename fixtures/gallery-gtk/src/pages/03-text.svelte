<!-- kinds: Entry PasswordEntry SearchEntry Text EditableLabel Label Inscription TextView ShortcutLabel -->
<script>
  let entry = $state('entry');
  let pass = $state('secret');
  let search = $state('find');
  let text = $state('text');
  let editing = $state(false);
  let view = $state('text view');
  let eView = $state(0);
  let eEntry = $state(0);
  let ePass = $state(0);
  let eSearch = $state(0);
  let eText = $state(0);
  let eEditing = $state(0);
  let activated = $state(0);
  let searched = $state(0);
  let link = $state('none');
  const editingEvents = { 'onnotify::editing': () => eEditing++ };
  $effect(() => {
    // programmatic writes after mount: must not come back as change events
    entry = 'entry 2';
    pass = 'secret 2';
    search = 'find 2';
    text = 'text 2';
    editing = true;
    view = 'text view 2';
  });
</script>

<box orientation="vertical" spacing="4">
  <label label="Label: markup, wrap, selectable" useMarkup={true} wrap={true} selectable={true}
    justify="center" xalign={0.5} lines={2} maxWidthChars={40}
    onactivate-link={(e) => { link = 'link'; e.preventDefault(); }} />
  <label label="link handler: {link}" />
  <entry bind:text={entry} placeholder="entry" visibility={true} maxLength={20} hasFrame={true}
    inputPurpose="free-form" activatesDefault={false} overwriteMode={false}
    onchanged={() => eEntry++} onactivate={() => activated++} />
  <label label="echo text: {eEntry}" />
  <label label="activate events: {activated}" />
  <passwordentry bind:text={pass} placeholder="password" showPeekIcon={true}
    onchanged={() => ePass++} />
  <label label="echo password: {ePass}" />
  <searchentry bind:text={search} placeholder="search" searchDelay={100}
    onchanged={() => eSearch++} onsearch-changed={() => searched++} />
  <label label="echo search: {eSearch}" />
  <label label="search-changed events: {searched}" />
  <text bind:text={text} placeholder="text" maxLength={30} onchanged={() => eText++} />
  <label label="echo plain text: {eText}" />
  <editablelabel bind:editing={editing} {...editingEvents} />
  <label label="echo editing: {eEditing}" />
  <inscription text="inscription" minLines={1} natChars={20} />
  <shortcutlabel accelerator="<Control>q" />
  <textview bind:text={() => view, (v) => { view = v; eView++; }} editable={true} wrapMode="word" monospace={true} cursorVisible={true}
    leftMargin={4} topMargin={4} justification="left" vexpand={true} />
  <label label="echo text view: {eView}" />
</box>
