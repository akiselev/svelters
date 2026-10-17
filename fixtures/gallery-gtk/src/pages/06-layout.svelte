<!-- kinds: Stack StackSwitcher StackSidebar Notebook FlowBox FlowBoxChild ListBox ActionBar Grid Fixed Overlay -->
<script>
  let page = $state(0);
  let pageEchoes = $state(0);
  // GTK switches to the first page by itself when the notebook is mapped: that first native
  // notify::page is not an echo of the app's write, so it is skipped
  let settled = false;
  let rows = $state(0);
  let picked = $state(0);
  $effect(() => {
    // a programmatic write after mount: must not come back as a change event
    page = 1;
  });
</script>

<box orientation="vertical" spacing="4">
  <stack transition-type="crossfade" transition-duration={0} hhomogeneous={true} vhomogeneous={false}>
    <label parent:name="one" parent:title="One" label="stack child one" />
    <label parent:name="two" parent:title="Two" label="stack child two" />
  </stack>
  <stackswitcher />
  <stacksidebar />
  <notebook bind:page={page} show-tabs={true} show-border={true} tab-pos="top" scrollable={false} {...{ 'onnotify::page': () => { if (settled) pageEchoes++; else settled = true; } }}>
    <label parent:tabLabel="First" label="page one" />
    <label parent:tabLabel="Second" parent:reorderable={true} label="page two" />
    <label parent:tabLabel="Third" label="page three" />
  </notebook>
  <label label="echo page: {pageEchoes}" />
  <flowbox selection-mode="single" min-children-per-line={2} max-children-per-line={4} homogeneous={true} column-spacing={4} row-spacing={4} onchild-activated={() => picked++} onselected-children-changed={() => picked++}>
    <label label="flow a" />
    <label label="flow b" />
    <label label="flow c" />
  </flowbox>
  <listbox selection-mode="single" show-separators={true} activate-on-single-click={true} onrow-activated={() => rows++} onrow-selected={() => rows++}>
    <label label="row one" />
    <label label="row two" />
  </listbox>
  <actionbar revealed={true}>
    <label label="start" />
    <label data-slot="center" label="center" />
    <label data-slot="end" label="end" />
  </actionbar>
  <grid row-spacing={2} column-spacing={2} row-homogeneous={false} column-homogeneous={false}>
    <label parent:column={0} parent:row={0} label="cell 0,0" />
    <label parent:column={1} parent:row={0} label="cell 1,0" />
    <label parent:column={0} parent:row={1} parent:columnSpan={2} label="spans two columns" />
  </grid>
  <fixed>
    <label parent:x={10} parent:y={4} label="fixed at 10,4" />
  </fixed>
  <overlay>
    <label label="overlay base" />
    <label data-slot="overlays" parent:measure={false} parent:clip={true} label="overlay" halign="end" valign="start" />
  </overlay>
</box>
