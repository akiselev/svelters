# skelter: why does a node look like this?

`Document::explain_style(node, property)` answers "where does this value come from?" for one node and one
property. It is the read behind an inspector's computed-style pane: click a value, see the rule that set it, the rules
that lost and why, and the token chain behind a `var()`. Engine side: `svelters_style::Stylist::explain`. Milestone
record: `plans/milestones/SV88-cascade-origins.md`.

It costs nothing unless called. The restyle path keeps no extra data for it; each call re-matches the node (and, for
inheritance and `var()`, its ancestors) against the current sheets. Call it from tools, not every frame.

## What it returns

`Explanation<NodeId>` (re-exported from `skelter_core`):

| Field | Meaning |
| --- | --- |
| `property` | What was asked: a longhand (`background-color`) or a custom property (`--sv-accent`). A shorthand declaration (`padding`) counts for each longhand it sets. |
| `winner` | The declaration the value comes from on this node, or `None` when nothing on the node declares it (or every declaration was rolled back). |
| `beaten` | Every other declaration of the property that applies to the node, **highest first** (the runner-up first), each with a `LossReason`. |
| `specified` | The winner's value after `var()` substitution (a shorthand's part for the longhand). `None` when there is no winner, or when substitution failed: the value is invalid at computed-value time and acts as `unset`. For a custom property: its computed value. |
| `computed` | The computed value from the node's last restyle (`None`: not set, the kind's native default). |
| `inherited_from` | When there is no winner (or it says `inherit`/`unset`) and the property inherits: the nearest ancestor whose own declaration supplies it. Ask `explain_style` on that node to continue. |
| `vars` | For each `var()` in the winner's value (outside fallbacks), a `VarResolution`. |

### `CascadeDecl`: one declaration

- `origin`: `DeclOrigin::{Theme, Author, User, Inline}`. The theme sits where CSS has the user-agent sheet; see
  [tokens-and-origins.md](tokens-and-origins.md) for the order (theme < author < user < inline; for `!important`
  author < inline < theme < user).
- `rule` and `sheet`: the stylist's ids (`None` for inline; `sheet` is `None` for a generated utility rule too).
- `sheet_name` and `source`: what the host told `Document::set_sheet_info` (a component's logical name; in dev builds,
  the file and offset in its `<style>`).
- `selectors`, `layer`, `specificity`: the rule's selector text, `@layer` name and the highest matched specificity.
- `important`, `name` (the declared name, possibly a shorthand), `value` (as written), `line`/`column` (the
  declaration's position in the sheet text) and `rule_line`/`rule_column`.

### `LossReason`: why a declaration lost

Each beaten declaration is compared with the winner, and the first cascade step where they differ is named:

| Reason | The winner is ahead because |
| --- | --- |
| `Importance` | one is `!important` and the other isn't (importance changes the origin order, so an `!important` theme rule beats a normal inline declaration) |
| `Origin` | same importance, a stronger origin |
| `Layer` | same origin and importance, a later `@layer` (an earlier one for `!important`; unlayered beats layered for normal declarations) |
| `Specificity` | same layer, a more specific selector |
| `Order` | everything equal, it comes later (sheets in registration order) |
| `Rollback` | this declaration sorted above the winner but is `revert`/`revert-layer`, which hand the property to the cascade below |
| `RolledBack` | this declaration sorted above the winner, but a `revert`/`revert-layer` above it excluded its origin or layer |

### `VarResolution`: a token chain

- `name`: `--name`.
- `declared_at`: the node whose declaration gives the value: the node using it, or the ancestor it inherits from.
  `None` when nothing declares it (the `var()` takes its fallback, or the value is invalid).
- `decl`: that declaration, with its origin and rule, so a user's override of a theme token shows as `User` here.
- `value`: the custom property's computed value at the node that uses it; `None` when it is guaranteed-invalid (unset,
  `initial`, or in a cycle).
- `uses`: the custom properties *that* declaration uses, resolved at `declared_at`, down to a literal.
- `cycle`: the chain came back to a property already on it; `uses` stops there.

## Example

A theme sheet `:root { --accent: #0000ff } box { background-color: var(--accent) }`, a user sheet
`#a { --accent: #00ff00 }`, and a child `b` of `#a` with `style="background-color: var(--accent)"`:

```rust
let e = doc.explain_style(b, "background-color").unwrap();
// e.winner: Inline, value "var(--accent)"
// e.beaten: [theme `box` rule, LossReason::Origin]
// e.specified: Some("#00ff00")
// e.vars[0]: --accent, declared_at Some(a), decl from the user sheet `#a`, value "#00ff00"
```

On `#a` itself, `explain_style(a, "--accent")` names the user rule as the winner; on the window root it names the
theme's `:root` rule.

## Limits

- Paint parts (`::part(...)`) are not explained yet; the node's own style is.
- A prop the runtime set on a kind (rather than CSS) still wins over the cascade, as everywhere (CSS.md §2.8); this
  read only covers what CSS decides.
- `vars` follows the references outside fallbacks. When a reference is unset, its `VarResolution` says so and the
  fallback's own references are not expanded.
