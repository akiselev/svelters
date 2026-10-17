# a1 gtk-display

- GDateTime args in dynamic calls are epoch ms numbers (not ISO strings); boxed props (Calendar.date) have no generated prop test.
- gen_backend failure leaves generated verify.rs stale: rerun after fixing, do not trust old test results.
- Event names with colons/hyphens in Svelte need spread: {...{"onnotify::day": fn}}.
- Video/MediaControls skipped (media streams, G7); DrawingArea resize signal has no easy trigger.
