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

<box-layout direction="top-to-bottom" spacing="6">
  <push-button text="Flat default push button" flat={true} auto-default={false} default={false} onclicked={() => clicks++} />
  <label text="clicks: {clicks}" />
  <check-box text="Check box" bind:checked={checked} ontoggled={() => echoChecked++} />
  <label text="echo checked: {echoChecked}" />
  <check-box text="Tristate" tristate={true} check-state="partially-checked" onstate-changed={() => states++} />
  <label text="state changes: {states}" />
  <radio-button text="Radio button" bind:checked={radio} ontoggled={() => echoRadio++} />
  <label text="echo radio: {echoRadio}" />
  <tool-button text="Tool button" auto-raise={true} tool-button-style="text-only" arrow-type="no-arrow" popup-mode="delayed-popup" />
  <command-link-button text="Command link" description="Explains what happens next" flat={false} />
</box-layout>
