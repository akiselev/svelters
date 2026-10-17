<script>
  // Qt twin of fixtures/styles (Qt vocabulary).
  import { setColorScheme, setTokens } from '@svelters/renderer/style';

  let on = $state(false);
  let dark = $state(false);
  let flips = $state(0);
</script>

<main-window window-title="Styles" class="app">
  <box-layout direction="top-to-bottom" spacing="8" class="card">
    <label class="title" class:big={on} style:color={on ? '#c01c28' : null}>Styled</label>
    <push-button class="primary" class:active={on} id="toggle" onclicked={() => (on = !on)}>Toggle</push-button>
    <line-edit class="field" placeholder-text="type here" />
    <push-button
      onclicked={() => {
        dark = !dark;
        setColorScheme(dark ? 'dark' : 'light');
      }}>Theme</push-button>
    <push-button
      onclicked={() => {
        flips++;
        setTokens({ '--sv-accent': flips % 2 ? '#26a269' : '#e5a50a' });
      }}>Accent</push-button>
  </box-layout>
</main-window>

<style>
  .app { background-color: var(--sv-bg); }
  .card { margin: var(--sv-space-3); padding: var(--sv-space-3); background-color: var(--sv-surface); border: 1px solid var(--sv-border); border-radius: var(--sv-radius); }
  .title { color: var(--sv-fg); font-size: 14pt; }
  .title.big { font-weight: bold; }
  push-button.primary { background-color: var(--sv-accent); color: var(--sv-accent-fg); border-radius: 6px; padding: 6px 14px; }
  push-button.primary:hover { background-color: var(--sv-accent-hover); }
  push-button.primary.active { border: 2px solid var(--sv-danger); }
  line-edit.field { color: var(--sv-fg); }
  line-edit.field:focus { border-color: var(--sv-accent); }
  @media (prefers-color-scheme: dark) {
    .card { border-color: #666; }
  }
  @native qt {
    .QPushButton { min-height: 24px; }
  }
</style>
