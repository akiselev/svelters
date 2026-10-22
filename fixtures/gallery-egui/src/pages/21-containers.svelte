<!-- kinds: Panel Sides Frame Stack -->
<script>
  // WG2b containers: a resizable side panel next to a body, sides, a captioned frame, and a stack
  // switched by name (the children's `anchor`).
  let size = $state(140);
  let page = $state('one');
</script>

<box gap={8}>
  <box flex-direction="row" height={80}>
    <panel side="left" resizable min-width={80} max-width={300} bind:size padding={6}>
      <label>Panel {size}</label>
    </panel>
    <box flex-grow={1} padding={6}>
      <label>Body</label>
    </box>
  </box>

  <sides align-items="center">
    <label>Left side</label>
    <button>Right side</button>
  </sides>

  <frame label="Settings" gap={4}>
    <label>Inside the frame</label>
    <checkbox>An option</checkbox>
  </frame>

  <box flex-direction="row" gap={6}>
    <button onclicked={() => (page = 'one')}>Show one</button>
    <button onclicked={() => (page = 'two')}>Show two</button>
  </box>
  <stack visible-child-name={page} align-self="start">
    <label anchor="one">Stack page one</label>
    <label anchor="two">Stack page two</label>
  </stack>
</box>
