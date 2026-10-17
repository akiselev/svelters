<!-- kinds: Image Picture Separator Spinner Calendar DrawingArea GraphicsOffload -->
<script>
  let spinning = $state(false);
  let spinEchoes = $state(0);
  let day = $state(1);
  let dayEchoes = $state(0);
  let selected = $state(0);
  $effect(() => {
    spinning = true;
    day = 15;
  });
</script>

<box orientation="vertical" spacing="6">
  <box orientation="horizontal" spacing="6">
    <image iconName="dialog-information" pixelSize={32} />
    <image iconName="edit-find" pixelSize={24} />
    <spinner bind:spinning {...{ "onnotify::spinning": () => spinEchoes++ }} />
    <label label="echo spinning: {spinEchoes}" />
  </box>
  <separator orientation="horizontal" />
  <box orientation="horizontal" spacing="6">
    <picture alternativeText="no file" canShrink={true} contentFit="contain" />
    <drawingarea contentWidth={40} contentHeight={20} />
    <graphicsoffload enabled="enabled" blackBackground={false} />
  </box>
  <calendar
    bind:day
    showHeading={true}
    showDayNames={true}
    showWeekNumbers={false}
    {...{ "onnotify::day": () => dayEchoes++ }}
    {...{ "onday-selected": () => selected++ }}
  />
  <label label="echo day: {dayEchoes}" />
  <label label="day {day}, selected events {selected}" />
</box>
