<window class="w"><entry class="e" /></window>

<style>
  .e { color: var(--sv-fg); }

  @native gtk {
    entry > text { color: red; }
    button.text-button:hover { box-shadow: inset 0 0 0 1px @accent_color; }
  }

  @native qt {
    .QLineEdit { selection-background-color: ${--sv-accent}; }
    QPushButton::menu-indicator { image: none; }
  }
</style>
