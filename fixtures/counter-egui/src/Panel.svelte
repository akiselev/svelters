<script>
  // AU2b: the component `mount` tests mount alone into a fresh host (E2E-INSPECTOR §2.4):
  // packages/test/test/fixture.e2e.js. A `title` prop and its own clicks, so `setProps` must keep
  // the state while the label changes.
  let { title = 'Panel' } = $props();
  let clicks = $state(0);
</script>

<box class="column">
  <button onclicked={() => clicks++}>Inc {title}</button>
  <label>{title} clicks: {clicks}</label>
</box>

<style>
  .column {
    gap: 8px;
  }
</style>
