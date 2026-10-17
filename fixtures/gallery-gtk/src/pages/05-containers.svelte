<!-- kinds: CenterBox Paned Frame Expander Revealer AspectFrame Overlay WindowHandle Viewport ScrolledWindow SearchBar -->
<script>
  let expanded = $state(false);
  let echoes = $state(0);
  let reveal = $state(true);
  $effect(() => {
    // a programmatic write after mount: must not come back as a change event
    expanded = true;
  });
</script>

<box orientation="vertical" spacing="4">
  <center-box>
    <label data-slot="start" label="start" />
    <label data-slot="center" label="center" />
    <label data-slot="end" label="end" />
  </center-box>
  <paned position={120} wide-handle={true} resize-start-child={true} shrink-end-child={false} hexpand={true}>
    <label data-slot="start" label="paned start" />
    <label data-slot="end" label="paned end" />
  </paned>
  <frame label="Frame" label-xalign={0.5}>
    <label label="in frame" />
  </frame>
  <expander label="Expander" bind:expanded={expanded} use-markup={false} onactivate={() => {}} {...{ 'onnotify::expanded': () => echoes++ }}>
    <label label="expander body" />
  </expander>
  <label label="echo expanded: {echoes}" />
  <revealer reveal-child={reveal} transition-type="slide-down" transition-duration={0}>
    <label label="revealed" />
  </revealer>
  <aspect-frame ratio={2.0} obey-child={false} xalign={0.5}>
    <label label="aspect" />
  </aspect-frame>
  <overlay>
    <label label="overlay base" />
    <label data-slot="overlays" label="over" halign="end" valign="start" />
  </overlay>
  <window-handle><label label="window handle" /></window-handle>
  <scrolled-window hscrollbar-policy="never" vscrollbar-policy="automatic" has-frame={true} min-content-width={200} min-content-height={40} propagate-natural-height={false}>
    <viewport><label label="viewport content" /></viewport>
  </scrolled-window>
  <search-bar search-mode-enabled={true} show-close-button={true}>
    <entry placeholder-text="search" />
  </search-bar>
</box>
