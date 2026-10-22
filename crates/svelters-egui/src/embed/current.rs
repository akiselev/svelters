//! Deferred viewports (ARCH-EMBED §5.2, EM2): the thread-local registry, `SlotPainter`,
//! `with_current` and the begin-pass plugin. Empty in EM1: EM1's hosts call `Embed::logic` from
//! `App::logic` and `show_slot` from the root viewport and immediate viewports. Writer lane: EMBED.
