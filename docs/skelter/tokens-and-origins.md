# skelter: tokens, the user origin and cheap theme changes

How a theme's tokens reach skelter's kinds, how a host layers a user's style sheets over the app's,
and what a token change costs. Milestone record: `plans/milestones/MAL2-LOOK.md` (SV67–SV71).

## One reader path for tokens

A theme (`skelter_widgets::theme::Theme`) is a map of custom properties: `--vscode-*` keys, the
`--sv-<role>` roles and the `--sv-<kind>-<part>[-<state>]` component tokens, plus anything a host
adds. `Theme::css()` is the token sheet: one `:root` block in `@layer sv.tokens`. `BASE_CSS` gives
the stock kinds their look with `var()` of those tokens.

What a kind paints where no rule sets a property comes from its user-agent table (`paint::Ua`). A
UA entry `Token("button-bg")` resolves **through the cascade first**: the node's computed
`--sv-button-bg`, so the token sheet and any override in the cascade (an inline `--sv-x` on a
container, a user sheet, an edited token) reach UA defaults exactly as they reach CSS rules. A part
without a style of its own reads its host's. Only where no such property is computed (a document
without a token sheet, a foreign painter with `StyleView::ua_only`) does the token come from the
typed theme in service. Parsed values are cached per distinct value text.

So with a token sheet installed, the sheet is the source of truth; the `Theme` service still serves
code that paints without a node (`Theme::colors()`, `Theme::color(name)`) and the scheme.

## Component tokens stay aliases

A component token that follows its role is written into the sheet as `var()` of it
(`--sv-button-bg: var(--sv-button2-bg)`, `--sv-checkbox-bg: var(--sv-control-bg)`). Overriding the
role in the cascade then reaches every component that follows it. The typed views
(`Theme::components()`) and `Theme::tokens()` keep the resolved values; `Theme::sheet_tokens()` is
what the sheet declares. A component that is overridden itself is written as its literal value.

## Changing tokens without a sheet swap

`Document::set_root_tokens(sheet, &[(name, Some(value) | None)])` edits the custom properties of a
registered sheet in place. The rule index is not rebuilt and the document is not restyled as a
whole: the nodes the edited rules match restyle (the window roots, for a `:root` block), and the
change travels down only as far as it is read.

`skelter_widgets::host::set_theme_tokens(doc, sheet, theme)` is the usual way to use it: it edits
the sheet with `theme.sheet_changes(&current)` and replaces the service. A change of scheme, contrast
or density repaints everything, as `host::set_theme` does (that one is for documents without a
token sheet).

### What a custom-property change restyles

When a node's computed custom properties change, skelter-core passes the changed names down
instead of restyling the whole subtree. A descendant is restyled when

- one of its (or its parts') matched declarations references a changed name through `var()`,
- it declares custom properties of its own (its map has to be rebuilt), or
- its kind says it reads a changed one outside the cascade: `Kind::reads_custom_property(name)`.

Every other descendant only takes the new inherited custom map, without matching or computing.
skelter-widgets' kinds answer `reads_custom_property` from the UA tokens they actually read while
laying out, measuring and recording, so a `--sv-progress` change re-records progress bars and
nothing else. The trait's default answers `true`: a host kind that doesn't override it is correct
and is re-measured and re-recorded on every custom-property change that reaches it.
`FrameStats::restyled` counts the nodes a frame restyled.

## The user origin

`svelters_style::Origin::User` (`Document::add_sheet_with_origin(css, Origin::User)`) is for the
person using the app. Its ranks, lowest first:

| Normal declarations | `!important` declarations |
|---|---|
| theme < author < **user** < inline | author < inline < theme < **user** |

This departs from CSS on purpose. On the web a user sheet loses to the page's own rules; here the
"author" is the application, and the person customising it wins over it, as a desktop toolkit's user
style provider does. Inline declarations still beat user rules, because what code writes inline is
geometry it computed. A user `!important` beats everything.

`@layer` works inside every origin as usual. `revert` rolls a property back to the cascade without
its declaration's origin (an inline or author `revert` falls to the theme, a user `revert` to the
author and inline values, a theme `revert` to `unset`); `revert-layer` rolls back to the earlier
layers of the same origin, and from the lowest layer on to the origin below. Custom properties roll
back the same way.

## Listing tokens and explaining a value

- `theme::token_manifest()` lists every `--sv-*` token: its tier (`role`, `component`, `density`,
  `shape`, `set`), type, value in each built-in set and density, the role a component follows, and
  `doc`, what it is for (`theme::token_doc(name)`).
- `Theme::with_source(source, overrides)` tags each override with a name the host chooses (a settings
  layer, a theme file, a preview); `Theme::with` leaves overrides untagged.
  `Theme::override_source(name)` reads the tag.
- `Theme::explain(name)` says where a token's value comes from, one step at a time:
  `TokenOrigin::Override { source }`, `TokenOrigin::Follows(other_token)` (a role its `--vscode-*`
  key, a component its role) or `TokenOrigin::Builtin`.
