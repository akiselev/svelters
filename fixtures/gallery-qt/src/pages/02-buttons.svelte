<!-- kinds: QPushButton QCheckBox QRadioButton QToolButton QCommandLinkButton -->
<script>
  let checked = $state(false);
  let radio = $state(false);
  let echoChecked = $state(0);
  let echoRadio = $state(0);
  let states = $state(0);
  let clicks = $state(0);
  $effect(() => {
    // programmatic writes after mount: must not come back as change events
    checked = true;
    radio = true;
  });
</script>

<box orientation="vertical" spacing="6">
  <button label="Flat default push button" flat={true} autoDefault={false} default={false} onclick={() => clicks++} />
  <label label="clicks: {clicks}" />
  <checkbox label="Check box" bind:checked={checked} ontoggled={() => echoChecked++} />
  <label label="echo checked: {echoChecked}" />
  <checkbox label="Tristate" tristate={true} checkState="partially-checked" onstateChanged={() => states++} />
  <label label="state changes: {states}" />
  <radiobutton label="Radio button" bind:checked={radio} ontoggled={() => echoRadio++} />
  <label label="echo radio: {echoRadio}" />
  <toolbutton label="Tool button" autoRaise={true} toolButtonStyle="text-only" arrowType="no-arrow" popupMode="delayed-popup" />
  <commandlinkbutton label="Command link" description="Explains what happens next" flat={false} />
</box>
