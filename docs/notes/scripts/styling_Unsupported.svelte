<box class="a"><button class="b" data-x="1">x</button><label>y</label><scale/><entry/></box>
<style>
  .a { display: flex; gap: 4px; width: 100px; }
  button[data-x="1"] { color: red; }
  button + label { color: blue; }
  label:first-child { color: red; }
  .a:has(button) { padding: 2px; }
  scale::slider:hover { background: oklch(0.6 0.1 250); }
  scale::thumb { color: red; }
  entry:focus-visible { outline: 1px solid red; }
  .a { --local: red; & label { color: var(--local); } }
  button:not(:hover) { opacity: 0.8; }
  @media (min-width: 400px) { .a { padding: 0; } }
  @keyframes pulse { from { opacity: 0 } to { opacity: 1 } }
  label { animation: pulse 1s; letter-spacing: 1px; text-align: center; }
</style>
