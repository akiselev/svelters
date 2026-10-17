# A1 gtk-ranges notes

- Default GtkAdjustment is 0..0: every value fact needs a range first. Property `setup:` and `rust` triggers call `h.call_args(o, "set_range", ...)` (dynamic GI) or the gtk4-rs setter; all passed first try.
- Scale and Range have no `value` property: value facts are `dispatch: dynamic` methods (`set_range`, `set_value`, `get_value`, `set_increments`) on Range, tested through `verify_subject: scale`. `set_increments` is checked through gtk4-rs (`adjustment().step_increment()`), since facts on the borrowed GtkAdjustment are blocked (G12).
- ProgressBar `pulse` has no readable state; its expect is `true` (the call must not fail).
- LevelBar `offset-changed` is detailed; a listener without detail receives it from `add_offset_value`.
- Gallery: event names with hyphens go through spread `{...{'onoffset-changed': fn}}`; bind:value on spinbutton/levelbar uses function bindings to count echoes; the app sets the range from `$effect` via `bind:this` + `call('set_range', 0, 100)`.
- Not annotated: SpinButton `input`/`output` (G5), Scale `set_format_value_func`/marks (callbacks), Range `adjustment` object facts (G12), Scale `bind:value` (no value property; the adjustment is not reachable).
