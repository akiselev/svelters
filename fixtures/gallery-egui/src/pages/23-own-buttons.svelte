<!-- kinds: Button -->
<script>
  // OW-CSS: svelters' own `button` styled from CSS alone: its paint parts (`::part(label)`,
  // `::part(icon)`), per-corner radii, a per-side border, gradients and transitions (eased on hover,
  // instant when pressed: DESIGN R2/R3). tests/gallery.rs opens this page.
  let clicks = $state(0);
</script>

<box class="col">
  <box class="row">
    <button class="pill" onclicked={() => clicks++}>Pill</button>
    <button class="accent" icon="check" onclicked={() => clicks++}>Accent</button>
    <button class="tab" onclicked={() => clicks++}>Tab</button>
    <button class="glow" onclicked={() => clicks++}>Glow</button>
  </box>
  <label>clicks: {clicks}</label>
</box>

<style>
  .col {
    gap: 8px;
  }
  .row {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }
  .pill {
    border-radius: 12px;
    padding-left: 14px;
    padding-right: 14px;
  }
  .pill::part(label) {
    text-transform: uppercase;
    letter-spacing: 1px;
    font-weight: 700;
  }
  .accent {
    background-image: linear-gradient(#3b8eea, #1f6fbf);
    color: #ffffff;
    border-color: #1a5ea3;
    transition: background-image 120ms ease, box-shadow 120ms ease;
  }
  .accent:hover {
    background-image: linear-gradient(#4f9df0, #2a7fd0);
  }
  .accent:active {
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.35);
    transition: none;
  }
  .accent::part(icon) {
    color: #c8f7c5;
  }
  .tab {
    border-width: 0;
    border-top: 2px solid #3b8eea;
    border-top-left-radius: 4px;
    border-top-right-radius: 4px;
    border-bottom-left-radius: 0;
    border-bottom-right-radius: 0;
  }
  .glow {
    background-image: radial-gradient(circle at 50% 40%, #ffe9a8, #c98a00);
    color: #2b1d00;
  }
  .glow:hover::part(label) {
    text-decoration-line: underline;
  }
</style>
