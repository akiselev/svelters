<!-- kinds: NSSlider NSProgressIndicator NSLevelIndicator NSDatePicker NSColorWell NSPathControl NSImageView -->
<script>
  import { NSColor } from 'svelters:appkit/appkit';

  let level = $state(30);
  let echoLevel = $state(0);
  const teal = NSColor.colorWithSRGBRedGreenBlueAlpha(0.1, 0.6, 0.6, 1);
  globalThis.__galleryKeep = [teal];
  $effect(() => {
    level = 60;
  });
</script>

<stack-view orientation="vertical" alignment="leading" spacing="8">
  <stack-view orientation="horizontal" spacing="8">
    <slider min-value="0" max-value="100" number-of-tick-marks={11} allows-tick-mark-values-only={true} bind:double-value={level} onaction={() => echoLevel++} />
    <text-field string-value={`level ${Math.round(level)}`} editable={false} bezeled={false} draws-background={false} />
    <text-field string-value={`echo level: ${echoLevel}`} editable={false} bezeled={false} draws-background={false} />
  </stack-view>
  <progress-indicator style="bar" indeterminate={false} min-value="0" max-value="100" double-value={level} />
  <progress-indicator style="spinning" indeterminate={true} displayed-when-stopped={true} />
  <level-indicator level-indicator-style="continuous-capacity" min-value="0" max-value="100" warning-value="70" critical-value="90" double-value={level} />
  <level-indicator level-indicator-style="rating" min-value="0" max-value="5" editable={true} double-value="3" />
  <date-picker date-picker-style="text-field-and-stepper" date-picker-elements={['year-month-day', 'hour-minute']} />
  <stack-view orientation="horizontal" spacing="8">
    <color-well color-well-style="default" color={teal} />
    <color-well color-well-style="minimal" supports-alpha={true} />
  </stack-view>
  <path-control path-style="standard" url={'/Applications/Utilities'} />
  <stack-view orientation="horizontal" spacing="8">
    <image-view image={'star.fill'} image-scaling="image-scale-proportionally-up-or-down" image-frame-style="none" />
    <image-view image={'NSFolder'} image-frame-style="gray-bezel" />
  </stack-view>
</stack-view>
