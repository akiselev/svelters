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
    <image icon-name="dialog-information" pixel-size={32} />
    <image icon-name="edit-find" pixel-size={24} />
    <spinner bind:spinning {...{ "onnotify::spinning": () => spinEchoes++ }} />
    <label label="echo spinning: {spinEchoes}" />
  </box>
  <separator orientation="horizontal" />
  <box orientation="horizontal" spacing="6">
    <picture alternative-text="no file" can-shrink={true} content-fit="contain" />
    <drawing-area content-width={40} content-height={20} />
    <graphics-offload enabled="enabled" black-background={false} />
  </box>
  <calendar
    bind:day
    show-heading={true}
    show-day-names={true}
    show-week-numbers={false}
    {...{ "onnotify::day": () => dayEchoes++ }}
    {...{ "onday-selected": () => selected++ }}
  />
  <label label="echo day: {dayEchoes}" />
  <label label="day {day}, selected events {selected}" />
</box>
