<!-- kinds: Stack StackSwitcher StackSidebar Notebook FlowBox FlowBoxChild ListBox ActionBar Grid Fixed -->
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
    <label label="stack child one" />
    <label label="stack child two" />
  </stack>
  <stackswitcher />
  <stacksidebar />
  <notebook bind:page={page} show-tabs={true} show-border={true} tab-pos="top" scrollable={false} {...{ 'onnotify::page': () => { if (settled) pageEchoes++; else settled = true; } }}>
    <label label="page one" />
    <label label="page two" />
    <label label="page three" />
  </notebook>
  <label label="echo page: {pageEchoes}" />
  <flowbox selection-mode="single" min-children-per-line={2} max-children-per-line={4} homogeneous={true} column-spacing={4} row-spacing={4} onchild-activated={() => picked++} onselected-children-changed={() => picked++}>
    <label label="flow a" />
    <label label="flow b" />
    <label label="flow c" />
  </flowbox>
  <listbox selection-mode="single" show-separators={true} activate-on-single-click={true} onrow-activated={() => rows++} onrow-selected={() => rows++}>
  </listbox>
  <actionbar revealed={true}>
    <label label="start" />
    <label data-slot="center" label="center" />
    <label data-slot="end" label="end" />
  </actionbar>
  <grid row-spacing={2} column-spacing={2} row-homogeneous={false} column-homogeneous={false} />
</box>
