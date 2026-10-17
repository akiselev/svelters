# ST1: merge PR #3 (native compiler) and unify its styling into X6

## Goal

PR #3 (`claude/sleepy-thompson-ge3jhj`, NC1) adds `svelters-compiler`, a Rust Svelte compiler. It also adds `@svelters/native-runtime`, a second frontend that runs beside stock Svelte, Vite and `@svelters/renderer`.

PR #3 and X6 each built a styling system:
- **X6:** scoped `<style>` is translated to GTK CSS or QSS through the v2.3 `style` op, with real native classes.
- **PR #3:** `<style>` is lowered at compile time to `setProp` calls. Its vocabulary is tiny and CSS-like (`min-width`, `enabled`, `emphasis`), and classes are never sent to the widget.

ST1 lands PR #3 with the two systems merged into one, built on X6.

## Rules (user, 2026-10-17)

- **No portability layer between frameworks.** No CSS-standard aliases and no cross-toolkit abstractions such as `emphasis`.
- **Property names follow the native API, converted to kebab-case.**
  - Setter properties use the native property name: GTK `width-request`, `hexpand`, `margin-start`, `sensitive`; Qt `minimum-width`, `maximum-height`, `contents-margins`.
  - Native-styling properties keep their dialect's own names (GTK CSS and QSS are native), so `color` and `background` stay.
- **Each property has a mapping entry that says how it is applied:**
  - `css`: GTK CSS or QSS (the X6 path, unchanged);
  - `setter`: a widget property set through `setProp`;
  - `child-attr`: `setChildAttr` on the parent, for example the Qt layout stretch and alignment.

  Layout properties (`spacing`, `homogeneous`, `margin-*` on GTK, h/v expand, h/v align, size requests and minimum/maximum sizes) and any other mapped property are setters.
- **Future WinUI and UIKit backends use setters for everything.** Their selectors are limited to type, `.class`, and `#id` where the platform supports it. The table format must allow a `setter`-only toolkit.

## Design

1. **Mapping source.** It lives in `metadata/style/{gtk,qt}.yaml`, hand-written and per toolkit like the overrides.
   - It replaces X6's `MATRIX`/`LAYOUT_HINTS` in `packages/style/src/vocab.js` and the `style:` sections of PR #3's `metadata/semantics/rules/*.yaml`.
   - Each entry gives:
     - `via`;
     - for setters, the target prop or child attribute, the value type, unit handling, and which kinds it applies to.
   - The generator checks every setter target against the verified metadata. A property that isn't writable on the kind is an error.
   - Generated outputs:
     - `packages/style/src/props.generated.js`;
     - the data the compiler's semantic spec needs;
     - the docs matrix.
2. **Declarations are split per property.**
   - `css` declarations go to the X6 translator as today.
   - `setter` and `child-attr` declarations become setter rules.
   - The selector of a setter rule must be a compound of type, `.class` and `#id`. Anything else is a diagnostic that names the property and says that setter properties only take type, class and id selectors.
   - `:state`, `@media` and `@native` stay CSS-only.
3. **Setter rules at runtime on the stock Svelte path.**
   - The style plugin registers setter rules next to the sheets, through `registerStyles(id, {gtk, qt, qtDark, rules})`.
   - The renderer matches rules when a node is created and whenever its `class` or `id` changes; this covers `class:`.
   - It applies the difference in values. Before a rule first overrides a prop, the renderer captures the prop's original value with `getProp`, and it restores that value when no rule sets the prop any more.
   - Precedence: explicit attribute > `style:` / `style=` > rules (specificity, then source order) > native default.
   - `style:spacing={8}` and `style="spacing: 8"` with setter properties go through the same path.
4. **The compiled path (PR #3).**
   - Keep its compile-time matching, but read the unified table instead of the vocabulary in `rules/*.yaml`.
   - Classes must reach the widget through `style(CLASSES)`, just as the renderer sends them, so that X6 sheets and `@native` blocks match.
   - `css` declarations are forwarded to the X6 translator rather than rejected, which also means accepting `style:` / `style=`.
   - Drop the `emphasis` intent and `$n.token`'s write of the whole `cssClasses` list. On GTK, users write `class="suggested-action"`; on Qt, `default={true}`.
   - Its selector support for setter properties should match the stock path, so drop combinators there.
5. **GTK `css-classes`.** Only X6's CLASSES op touches them, by diffing add and remove. Nothing writes the whole list.
6. **Docs.**
   - The doc generator emits a per-toolkit styling reference from the table, listing each property with whether it is applied through native CSS/QSS or a setter (and which setter).
   - The styling guide explains how each platform works: the GTK CSS node names and classes, QSS with the dynamic `class` property, setter rules, precedence and restoring, and the plan for WinUI and UIKit (setters only, simple selectors).
   - The existing docs test checks the guide against the table.
7. **Gates.**
   - `pnpm test` must not require a prebuilt `svelters-compile`. The compiled experiment either builds it through cargo-slot in its own `test:compiler` script or skips with a clear message, and the gate documents which.
   - `gen_semantics.py --check` joins `test:meta`.

## Steps

1. Merge `pr-3` into the lane branch. Get the PR's own tests and every master gate green before changing any design: `pnpm test`, `pnpm docs:test`, and nextest through cargo-slot.
2. Mapping YAML, its schema, the generator and the generated tables, with the old `vocab.js` matrix removed.
3. Renderer setter rules, including `class:`, `id` and `style:` setters, with tests in testhost op logs.
4. Compiler changes from design item 4. The PR's `styled-toolbar` and `dynamic-class` fixtures are updated to the new names and behaviour.
5. Galleries: add one setter/layout section per toolkit to the existing styling gallery pages, with screenshot or op-log tests.
6. Docs from design item 6.
7. Update `plans/STATUS.md` and record notes in `docs/notes/st1-style-unification.md`.

## Out of scope

- **The tag rename to native kebab-case names without the prefix** (QMainWindow becomes `main-window`). That is the separate lane N1, after ST1 and X8.
- **Hard-coded tag names.** ST1 reads tag names from the tag table and hard-codes none.
