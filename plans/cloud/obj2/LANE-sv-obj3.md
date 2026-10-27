# Lane SV-OBJ3: a host engine in the selector grammar

Coordinator: cadatomic session `session_016CTvCHJyR4hx83LMP4XD5y` (send_message). svelters' `CLAUDE.md` applies: its five CI workflows are the gate (`tools/dev/ci-wait`), never run the full suite locally, never ask the user. Branch `ci/sv-obj3` from svelters master `4e9f4b62`. Record in `plans/milestones/SV-OBJ3.md` with the sha and CI run ids.

This is a request from cadatomic's object model work (generic: no host names or types). The request text follows.


For the svelters lane that takes it. Generic: no host names, no host types. Source: cadatomic's
`docs/research/2026-10-26-semantic-object-model.md` §4.3, §6.2; the cadatomic side is written against
this signature and gated on the pin (below).

## What

A selector part that hands its text to the host, so a host's own query language composes with the
role/anchor chain in one string:

```text
root=pane.board >> obj=ecad.pad[ref="R1.2"]
anchor=main.canvas >> obj=@view world:20mm,15mm
```

### `svelters-automation::selector`

- `Part::Host { engine: String, text: String }`.
- Parsed like `component=`: an engine prefix the host registers is not needed; `obj=` is the one
  engine name parsed as `Host` today (`engine: "obj"`). The text runs to the end of the part, as
  `css=`'s does; `>>` inside the text ends the part only outside brackets and quotes (a query has
  `[...]` and `"..."`).
- `Selector::parse` keeps the source text, so a host can re-read its part verbatim.

### `svelters-automation::source::SceneSource`

```rust
/// A part this engine does not know (`obj=`): the host answers it among `among` (the previous
/// part's matches, or every root's top when it is the first part). Default: refused with
/// `capability.absent`, so a source that has no host engine fails loudly.
fn host_select(
    &self,
    engine: &str,
    text: &str,
    among: &[Self::Node],
) -> Result<Vec<Self::Node>, Error> {
    let _ = (engine, text, among);
    Err(Error::new("capability.absent", format!("this scene answers no `{engine}=` part")))
}
```

- `source::query` calls it for `Part::Host`. The nodes it returns are ordinary nodes of the scene
  (a host returns the synthetic node it uses for an object that has no node of its own: the scene
  decides how it shows; svelters gets no object type).
- Nothing else changes: `Pos`, `Nth` and the other parts apply to its result as to any other.

### Tests (svelters)

- Parse: `obj=a[b="c >> d"] >> nth=0` is two parts, the first `Host { engine: "obj", text:
  "a[b=\"c >> d\"]" }`.
- A fixture scene whose `host_select` maps `obj=` text to node ids: a chain `anchor=x >> obj=q`
  passes only the nodes under `x` as `among`; a scene without the method refuses
  `capability.absent`.

## Done

svelters CI green on all five workflows; the record names the sha and run ids; one line for cadatomic's `docs/plan/04-kernel-requests.md`.
