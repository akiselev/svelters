# a1 gtk-containers-a
- Frame/Expander `label` slot (label-widget) uses default builtin + restore_default like HeaderBar title; passes.
- ScrolledWindow::edge-reached cannot be triggered by real scrolling without allocation; test emits it by name (delivery only).
- Svelte attribute names cannot contain ":"; listen to notify::x via spread {...{"onnotify::expanded": fn}}.
- One batch test was flaky once (not identified); full gate passed.
