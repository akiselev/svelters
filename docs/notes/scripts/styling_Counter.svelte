<script>
  let count = $state(0);
</script>

<window title="Counter">
  <box class="card" orientation="vertical" spacing="8">
    <label class="count" style="font-weight: bold" style:color={count > 2 ? '#c01c28' : null}>Count: {count}</label>
    <button class="primary" onclick={() => count++}>Increment</button>
  </box>
</window>

<style>
  .card {
    padding: 16px;
    background-color: var(--sv-surface);
    border: 1px solid var(--sv-border);
    border-radius: 8px;
  }
  .count { font-size: 20px; color: var(--sv-fg); }
  button.primary {
    background: var(--sv-accent);
    color: var(--sv-accent-fg);
    border-radius: 6px;
    padding: 6px 14px;
    transition: background-color 150ms ease;
  }
  button.primary:hover { background: var(--sv-accent-hover); }
  @media (prefers-color-scheme: dark) {
    .card { border-color: #555; }
  }
</style>
