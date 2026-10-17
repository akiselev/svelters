<window class="w">
  <box class="a b"><label class="x">x</label><label class="y">y</label></box>
  <checkbutton class="cb">c</checkbutton>
</window>

<style>
  .a { display: flex; width: 100px; text-align: center; color: red; }
  .a[data-x="1"] { color: blue; }
  .x + .y { color: green; }
  .x ~ .y { color: green; }
  .b { background-color: oklch(0.6 0.1 250); color: #123; }
  .b { box-shadow: 0 1px 2px #000; transition: color 150ms; }
  .b { --local: 1px; padding: calc(1px + 2px); margin: var(--other); }
  .x:first-child { color: #111; }
  .y:focus-visible { color: #222; }
  .y::before { color: #333; }
  .a { opacity: 0.8; font-family: serif; background: linear-gradient(red, blue); }
  .a:is(.x, .y) { color: pink; }
  checkbutton.cb { color: #444; }
  window.w { color: var(--sv-unknown); }
  @keyframes pulse { from { color: red; } to { color: blue; } }
  @media (min-width: 600px) { .a { color: red; } }
  @media (prefers-reduced-motion: reduce) { .a { color: red; } }
</style>
