# SV-OBJ3: a host engine in the selector grammar

Lane brief: `plans/cloud/obj2/LANE-sv-obj3.md`. Branch `ci/sv-obj3` from master `4e9f4b62`.

## Done

- `svelters-automation::selector`: `Part::Host { engine, text }`. `obj=` is the one engine parsed
  as `Host` today (`engine: "obj"`); `text` is everything after `obj=`, verbatim, to the end of the
  part. The existing chain splitter already ends a part at `>>` only outside brackets and quotes,
  so `obj=a[b="c >> d"] >> nth=0` is two parts. `Selector::parse` (new associated fn, same as
  `selector::parse`/`FromStr`) keeps the source text in `Selector::source`.
- `svelters-automation::source::SceneSource::host_select(engine, text, among)`, default refused
  with `capability.absent` ("this scene answers no `obj=` part"). The engine calls it for
  `Part::Host`; `among` is the previous part's matches (exactly those nodes, not their subtrees:
  the host scopes its own query under them), or every unskipped root's top when it is the first
  part. Its result (deduplicated, order kept) is an ordinary match set: the next part searches
  below it, `nth=` picks from it.
- Tests (unit, no features): `selector::tests::parses_chains_and_engines` (the parse case),
  `source::tests::host_engine_answers_among_the_previous_matches` (`anchor=x >> obj=…` passes
  `[x]` as `among`; first part passes the root tops; `nth=` and `root=` compose),
  `source::tests::host_engine_default_is_absent`.

Note on the brief: its test line says "passes only the nodes under `x`"; the signature's doc says
"the previous part's matches". I followed the signature (`among == [x]`), which lets the host
search under `x` itself. If cadatomic wants `x`'s subtree instead, it is a one-line change in
`engine.rs` (`candidates(..)` in place of `matched(..)`).

Run locally: the three tests above (`cargo test -p svelters-automation --lib`, filtered). Not run:
anything else; CI is the gate.

## CI

- sha: see below (filled after the run).

## Line for cadatomic's `docs/plan/04-kernel-requests.md`

SV-OBJ3 (host engine `obj=` in selectors): done in svelters `ci/sv-obj3` — `Part::Host { engine,
text }`, `SceneSource::host_select(engine, text, among)` (default `capability.absent`); `among` is
the previous part's matches or the root tops.
