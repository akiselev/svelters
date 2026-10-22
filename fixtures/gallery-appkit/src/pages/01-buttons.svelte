<!-- kinds: NSButton NSSwitch NSSegmentedControl NSPopUpButton NSStepper -->
<script>
  // Buttons and selection controls with their two-way bindings. The $effect writes after mount must not
  // come back as change events: every `echo` label stays 0 (examples/counter-mac/tests/gallery.rs).
  let on = $state('off');
  let segment = $state(0);
  let picked = $state(0);
  let steps = $state(1);
  let echoSwitch = $state(0);
  let echoSegment = $state(0);
  let echoPopup = $state(0);
  let echoStepper = $state(0);
  let clicks = $state(0);
  $effect(() => {
    on = 'on';
    segment = 2;
    picked = 1;
    steps = 4;
  });
</script>

<stack-view orientation="vertical" alignment="leading" spacing="8">
  <stack-view orientation="horizontal" spacing="8">
    <button title="Push" bezel-style="push" onaction={() => clicks++} />
    <button title="Destructive" bezel-style="push" has-destructive-action={true} onaction={() => clicks++} />
    <button title="Check" button-type="switch" state="mixed" allows-mixed-state={true} />
    <button title="Radio" button-type="radio" state="on" />
    <text-field string-value={`clicks: ${clicks}`} editable={false} bezeled={false} draws-background={false} />
  </stack-view>
  <stack-view orientation="horizontal" spacing="8">
    <switch bind:state={on} onaction={() => echoSwitch++} />
    <text-field string-value={`switch ${on}`} editable={false} bezeled={false} draws-background={false} />
    <text-field string-value={`echo switch: ${echoSwitch}`} editable={false} bezeled={false} draws-background={false} />
  </stack-view>
  <stack-view orientation="horizontal" spacing="8">
    <segmented-control labels={['Day', 'Week', 'Month']} segment-style="rounded" bind:selected-segment={segment} onaction={() => echoSegment++} />
    <text-field string-value={`segment ${segment}`} editable={false} bezeled={false} draws-background={false} />
    <text-field string-value={`echo segment: ${echoSegment}`} editable={false} bezeled={false} draws-background={false} />
  </stack-view>
  <stack-view orientation="horizontal" spacing="8">
    <pop-up-button item-titles={['Red', 'Green', 'Blue']} bind:index-of-selected-item={picked} onaction={() => echoPopup++} />
    <pop-up-button pulls-down={true} item-titles={['Actions', 'Copy', 'Paste']} />
    <text-field string-value={`picked ${picked}`} editable={false} bezeled={false} draws-background={false} />
    <text-field string-value={`echo popup: ${echoPopup}`} editable={false} bezeled={false} draws-background={false} />
  </stack-view>
  <stack-view orientation="horizontal" spacing="8">
    <stepper min-value="0" max-value="10" increment="1" value-wraps={false} bind:double-value={steps} onaction={() => echoStepper++} />
    <text-field string-value={`steps ${steps}`} editable={false} bezeled={false} draws-background={false} />
    <text-field string-value={`echo stepper: ${echoStepper}`} editable={false} bezeled={false} draws-background={false} />
  </stack-view>
</stack-view>
