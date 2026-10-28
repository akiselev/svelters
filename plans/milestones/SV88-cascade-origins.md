# SV88: the cascade's origin reads ("why does this node look like this?")

Branch `ci/sv-b4`, base `master` `38f3b47`. Lane brief: the coordinator's message to the session (no batch file).
Status: built and tested; CI below.

The request row was not yet on cadatomic's default branch (`1c03543`) when this lane started; the coordinator's brief
was the spec.

## What landed

**svelters-style** (`src/explain.rs`, new): `Stylist::explain(el, property, &dyn StyleLookup<E>) -> Explanation<E>`.

- `StyleLookup<E>`: the caller gives each element's inline block and computed style (the engine keeps neither).
- `Explanation<E>`: `winner: Option<CascadeDecl>`, `beaten: Vec<Beaten>` (highest first, each with a `LossReason`),
  `specified` (value after `var()` substitution; `None` = invalid at computed-value time), `computed`,
  `inherited_from: Option<E>`, `vars: Vec<VarResolution<E>>`; `map` (element handles to node ids) and `annotate`
  (fill sheet names and sources).
- `CascadeDecl`: `origin: DeclOrigin {Theme, Author, User, Inline}`, `rule`, `sheet`, `sheet_name`, `selectors`,
  `layer`, `specificity`, `important`, `name`, `value`, `line`/`column`, `rule_line`/`rule_column`, `source`.
- `LossReason {Importance, Origin, Layer, Specificity, Order, Rollback, RolledBack}`: the first cascade step on which
  the loser differs from the winner; the last two cover declarations above a winner reached through `revert` /
  `revert-layer`.
- `VarResolution<E>`: `name`, `declared_at` (the element or ancestor that declares it), `decl`, `value` (computed at
  the user), `uses` (recursive, resolved where declared), `cycle`.
- All new public types are `#[non_exhaustive]` except `Beaten`.

**skelter-core**: `Document::explain_style(node, property) -> Option<Explanation<NodeId>>` (`style/inspect.rs`),
filling sheet names and `<style>` sources from `set_sheet_info`. The types are re-exported from `skelter_core`.

Cost: none on the restyle path. The cascade keys are rebuilt inside `explain` (`property_cascade`, a copy of
`order_decls`' keys restricted to one property) instead of widening `Cascaded`, so the per-frame path carries nothing
new. `INLINE_RANK` became `pub(crate)` and skelter's `El::id` `pub(crate)`; nothing else changed in existing code.

Docs: `docs/skelter/why-this-style.md`.

## Tests

- `crates/svelters-style/tests/explain.rs`: a theme + author + user sheet with an inline style (winner, losers on
  origin, the user's override of a theme token through a two-step `var()` chain, a custom property's own winner and
  inheritance); each cascade step (order, specificity, layer, importance); `revert` (Rollback / RolledBack);
  inheritance from an ancestor; a value invalid at computed-value time; a `var()` cycle.
- `crates/skelter-core/src/tests/style.rs` `explain_style_names_origins_losers_and_token_chains`: the same through the
  document, with sheet names.

Run locally: those 4 tests only (pass), and `cargo clippy -p svelters-style -p skelter-core --all-targets` (nothing in the
new code). Not run: anything else; CI is the gate.

## Open / follow-ups

- `::part(...)` styles are not explained (the node's own style is). Cheap to add with a `pseudo` argument if asked.
- svelters-egui's scene has the same `Stylist` and could expose the same read; not done (maintenance only).
- `vars` does not expand references inside a fallback that was taken.

## CI

(filled in below after the push)
