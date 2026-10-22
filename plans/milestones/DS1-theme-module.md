# DS1: the theme module (`crates/svelters-egui/src/theme/`)

Design: `plans/research/DESIGN.md` §1.5 (token table), §1.6 (egui `Style` mapping), §2.5 (elevation), §2.6 (density),
§4.1–4.3 (layers, one map with three outputs, where the defaults live), §5 (row DS1). Built on CS5e
(`set_tokens`, the cascade), SG4, FX1 and INT1 on master. Lane: DS1 (STYLE writer), branch `ci/ds1`.

**Decided (lane, from the brief):** DESIGN.md Q1 is answered as recommended: the base is **Dark Modern / Light
Modern**; Dark 2026 / Light 2026 come later as themes through DS4's importer. The affordance overlay's token values are
**on by default** (§0, §4.3; Q2 is still open for the user, and flipping the default is one line in
`Theme::vscode`).

## What landed

| File | Content |
|---|---|
| `theme/mod.rs` (new) | `Theme` (recipe: scheme, `Contrast`, `Density`, affordances, overrides; the resolved `TokenMap`; typed views `Colors` (59 L2 roles), `Components` (16 L3 colours), `Metrics` (px), `Elevation` (egui `Shadow`s), `Fonts`, `Motion`; `problems()`), `TokenMap`, the tables `ROLES` (role → VS Code key), `COMPONENT_COLORS` / `COMPONENT_METRICS` / `COMPONENT_SHADOWS` (L3 → source), `vscode_var`, value parsers (`parse_color`, `parse_px`, `parse_shadow`), `contrast_ratio`, `BASE_CSS`, `LAYERS`. |
| `theme/egui_style.rs` (new) | `Theme::egui_style()` (DESIGN.md §1.6: Visuals, Spacing, text sizes, tooltip delay) and `Theme::apply_to(ctx)` (`set_style_of` + `set_theme`; keeps text styles the app added). |
| `theme/backend.rs` (new) | `EguiBackend::set_theme(&Theme)`, `theme()`, `theme_generation()`, `set_density(Density)`; the pending Style is applied at logic. |
| `theme/data/*.json` (new) | The four VS Code sets (`vscode-dark-modern`, `-light-modern`, `-hc-dark`, `-hc-light`: L1 colour keys plus the set's own role values, each with its source and notes), `shape.json` (scheme-independent tokens), `density.json` (3 sets), `affordance.json` (the overlay). |
| `theme/base.css` (new) | `@layer sv.base`: `button`, `checkbox`, `text-edit` (and `:hover`/`:active`) read `--sv-*` component tokens, restating the egui mapping for the stock-painted kinds. |
| `style.rs` (STYLE) | Host-owned sheets `@theme…` (theme origin; the runtime's clear keeps them). Activation installs the default theme's tokens (both schemes under `prefers-color-scheme`) unless a theme is installed. `SceneStyle::contrast` feeds `prefers-contrast`. `SVL_STYLE_SCHEME` and `set_color_scheme(Some(_))` switch an installed theme's scheme. `apply_sheet` is split into `install_sheet`/`uninstall_sheet`. |
| `backend.rs`, `frame.rs`, `lib.rs` (small) | `Inner::{theme, theme_generation, theme_pending}`; logic applies a pending theme before `restyle_pending`; `pub mod theme`. |
| `packages/native-runtime/src/style.js` | On an engine backend (egui) the runtime no longer sends its Adwaita `DEFAULT_TOKENS` or an unchosen scheme: `@tokens` carries only `setTokens` overrides (sent once there are any), the scheme only after `setColorScheme`. GTK, Qt and AppKit are unchanged. |
| `packages/native-runtime/test/style-engine.test.mjs` | Asserts the above (no defaults, no scheme, override-only `@tokens`, a chosen scheme reaches the backend). |
| `fixtures/{counter-egui,motion-egui,gallery-egui-menus}/expected.oplog` | The two init lines (`style scheme "light"`, the `@tokens` defaults) are gone. Edited by hand (the only change, deterministic); not regenerated locally, because that needs a `svelters-compile` build. CI's js job checks them. |
| `tests/theme.rs` (new) | The two kittest tests below. |

## API

```rust
use svelters_egui::theme::{ColorScheme, Contrast, Density, Theme};

let t = Theme::vscode(ColorScheme::Dark, Contrast::Normal)       // or dark_modern(), light_modern(), hc_dark(), hc_light()
    .with_density(Density::Compact)                               // Compact | Default (VS Code) | Comfortable
    .with_affordances(true)                                       // the overlay's tokens (default on)
    .with([("--sv-accent", "#FF8800"), ("--cad-panel", "#202020")]); // L1 `--vscode-*`, L2 `--sv-<role>`, L3, or pass-through
embed.backend().set_theme(&t);                                    // run time, no restart
embed.backend().set_density(Density::Comfortable);
embed.backend().set_color_scheme(Some(ColorScheme::Light));       // with a theme: the same recipe in light

// typed getters, for berth's DockStyle and quire's EditorTheme (filled by cadatomic)
t.colors().tab_active_bg; t.colors().selection; t.components().button_bg;
t.metrics().tab_h; t.metrics().row_h; t.elevation().popup; t.fonts().mono;
t.color("list-hover"); t.px("--sv-header-h"); t.shadow("elev-3"); t.token("--vscode-focusBorder");
t.css();           // the `:root` sheet, `@layer sv.tokens`
t.egui_style();    // egui::Style; t.apply_to(&ctx) for egui-only code
backend.theme_generation(); // bumped per set_theme, for shape caches
```

JS: the runtime's `setColorScheme('dark' | 'light')` switches an installed theme's scheme (the `SCHEME` op), and
`setTokens({...})` overrides tokens on top of the theme (an unlayered theme-origin sheet beats `@layer sv.tokens`). A
JS hook for contrast and density was not cheap (an export needs the JS thread's backend), so it is left out.

### Resolution and precedence

The VS Code set for (scheme, contrast) → `--vscode-*` overrides → roles from their VS Code keys → `shape.json`, the
density set, the set's own role values → the affordance overlay → `--sv-<role>` overrides → components from their
roles → component overrides. Other overrides pass through. In the cascade: the theme sheets are theme origin in
`@layer sv.tokens` / `sv.base` (DS3's `sv.affordance` is declared after them). The host's `set_tokens` sheets
(unlayered theme origin) and the runtime's `@tokens` overrides beat them, and author rules beat everything.

### Data decisions

- Values are VS Code 1.133.0's, read from the installed theme files (include chains resolved) and the colour registry
  in `workbench.desktop.main.js`. DESIGN.md's table was checked against them. One difference: Dark Modern's
  `list.dropBackground` is `#383B3D` in the theme chain, not the registry's `#062F4A`; the file wins.
- High Contrast keys that VS Code leaves null get role values in the set's `tokens`, each with a note: the secondary
  button, toolbar, drop, menu-selection and hover fills are derived from `list.hoverBackground`, so egui's hovered
  state still shows. VS Code outlines these with `contrastActiveBorder`, and DS3 adds the outlines. HC dark
  `selection` keeps the registry's `#F3F518`, because `hc_black.json`'s `#FFFFFF` needs a selection foreground that
  egui can't set.
- The affordance overlay adds dark `button2-hover: #3B3B3B` (not in DESIGN.md). With the raised `#313131` rest fill,
  Dark Modern's `#2B2B2B` hover would darken; `#3B3B3B` is VS Code's own derivation (lighten 0.2).
- `selection.stroke` (egui's selected-text and selected-button colour) is whichever of `fg-strong` and `surface` has
  the higher contrast on `selection`. egui also draws a focused TextEdit's border with it, so the focus colour there
  waits for DS3's focus helpers (or a `TextEditStyle` provider).

## Tests

- `tests/theme.rs::theme_switch_moves_stock_and_svelte_buttons_to_the_same_token`: one frame holds a stock
  `egui::Button` drawn from Rust and a scene `button` node. Before any theme, the backend's default `--sv-fg`
  resolves. Then the test switches Dark Modern → Light Modern → HC dark at run time. Each time, the context's
  `weak_bg_fill`, the stock button's painted fill, the Svelte button's computed `background-color` and its painted
  fill all equal `--sv-button-bg`. The `SCHEME` op switches HC dark to HC light. An `--sv-button-bg` override
  restyles both. On hover, both buttons take `--sv-button-bg-hover`.
- `tests/theme.rs::high_contrast_has_no_shadows` (R7): in both HC themes every `elev-*` token is `none` and the
  installed Style has `Shadow::NONE` for windows and popups; Dark/Light Modern do cast them.
- `theme::tests` (unit): every built-in set × density × affordance resolves every role, component, metric and
  elevation with no problems; overrides flow L1 → L2 → L3 and survive a scheme switch; value parsers.

Run locally (filtered): `cargo nextest run -p svelters-egui --test theme --test style --test oplog_replay` and
`--lib theme::` (all pass); `node --import ./test/register.mjs --test test/style*.test.mjs` in
`packages/native-runtime` (5 pass). Not run locally: the three egui fixtures through the Node TestHost (they need
`svelters-compile`), the gallery, snapshots, and everything else. CI covers those.

## Left for later lanes (the API is shaped for them)

- **DS2** engine gaps: `outline`, `cursor`, per-side borders, `transition`, `text-transform`, scroll shadows,
  disabled reasons. `base.css` grows with them (focus rings, the tab marker, cursors).
- **DS3** the affordance sheet: `@theme:affordance` in `@layer sv.affordance`, installed by `set_theme` when
  `theme.affordances()` (the slot is marked in `theme/backend.rs`). It also brings the elevation `box-shadow`s (CS5g),
  HC outlines, and the Rust helpers `focus_ring` / `paint_row_state`.
- **DS4** the VS Code theme importer: it produces `--vscode-*` overrides for `Theme::with` (the roles follow) and
  reads `Theme::problems()`, and it can derive the four sets' data from the defaults table that VIS1 step 0 captures.
- **DS5** codicons and semantic icon ids: tokens or a table beside the theme.
- **VIS1** tunes `theme/data/*.json` and `base.css` against the captured references.
- `packages/style`'s `DEFAULT_TOKEN_NAMES` (GTK/Qt/AppKit compile-time warnings) does not yet list the new roles. egui
  doesn't go through that check.
- cadatomic (through SV): map its `theme::Tokens` onto `--sv-*` (or replace them with `Theme`), and fill
  `DockStyle`/`EditorTheme` from the getters.
