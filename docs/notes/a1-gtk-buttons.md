# A1 gtk-buttons notes

- MenuButton `popover` slot dropped: the slot generator emits `set_popover(Some(child))` with `child: &gtk::Widget`, but gtk4-rs wants `IsA<Popover>`, and `get_popover()` returns `Option<Popover>` not `Option<Widget>`. Typed (non-Widget) slots do not compile. New gap: typed child slots (generator: downcast/upcast by `accepts`).
- MenuButton `popup`/`popdown` tests need a mapped, rooted widget: `setup` puts the button into a presented `gtk::Window` (leaked with `mem::forget`). `popup` is flaky on the first try (Gdk display not ready), passes on retry.
- ScaleButton `value` needs an adjustment with a non-empty range: the default adjustment is 0..0, so `setup` installs one; `value-changed` trigger does the same via `rust`.
- Switch has no `toggled`; binding uses `notify::active` (value: prop). `state-set` fires on programmatic `active` writes and is not suppressed (only `notify::active` is), so the gallery echo counter uses a function binding setter.
- Svelte cannot express `onnotify::active` attributes; use function bindings for echo counters on notify-only bindings.
- LinkButton `activate-link` trigger via `emit_clicked()` does not launch a URI in the test (the test handler stops it).
- Dropped/not attempted: CheckButton `group` (reference prop), ScaleButton `icons` (list<string>, not tried), LockButton `permission` (Gio.Permission object).
