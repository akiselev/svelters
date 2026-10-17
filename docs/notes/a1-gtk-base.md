# A1 gtk-base notes

- `opacity`: GTK quantises to 8 bits, so 0.5 reads back as 0.50196; the test sample is 0.0.
- `show` cannot use a `property` trigger on a label (it starts visible); a `rust` trigger hides then shows.
- `css-classes` (list<string>) has no generated property test: dropped.
- `select_region(1, 3)` on an Entry leaves cursor-position 1 and selection-bound 3 (not the reverse).
- `map`/`unmap` dropped: need a mapped toplevel, the harness creates none.
- `handler_result`/`sync_required` on Widget signals (query-tooltip, keynav-failed, mnemonic-activate): left as derived from GIR; no test trigger exists.
- `queue_draw`/`queue_resize`: no observable state; postcondition only checks the widget is intact.
