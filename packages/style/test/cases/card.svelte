<window class="app">
  <box class="card" orientation="vertical">
    <label class="count">Count</label>
    <button class="primary">Go</button>
    <toggle-button class="tog">T</toggle-button>
    <push-button class="primary">Go</push-button>
    <entry class="field" />
    <line-edit class="field" />
  </box>
</window>

<style>
  .app { background-color: var(--sv-bg); }
  .card {
    padding: var(--sv-space-3);
    background-color: var(--sv-surface);
    border: 1px solid var(--sv-border);
    border-radius: var(--sv-radius);
  }
  .count { font-size: 20px; font-weight: bold; color: var(--sv-fg, #222); }
  button.primary, push-button.primary { background: var(--sv-accent); color: var(--sv-accent-fg); border-radius: 6px; padding: 6px 14px; }
  button.primary:hover, push-button.primary:hover { background: var(--sv-accent-hover); }
  button.primary:active, push-button.primary:active { background: #1a5fb4; }
  button.primary:disabled, push-button.primary:disabled { opacity: 0.5; }
  toggle-button.tog:checked { background-color: #26a269; }
  .card > .count, .card .tog { margin: 4px; }
  entry.field:focus, line-edit.field:focus { border-color: var(--sv-accent); }
  .card:not(.count) { min-width: 120px; }
  @media (prefers-color-scheme: dark) {
    .card { border-color: #666; }
  }
</style>
