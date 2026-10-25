# Lane WAYLAND: presto-platform-wayland

Owns `crates/presto-platform-wayland/**`. smithay-client-toolkit and wayland-client. Deliver: xdg-shell
toplevels and popups; seat input (pointer with frame events and axis sources/`axis_stop`, keyboard via
xkbcommon with egui key names, touch, tablet); `text-input-v3` (surrounding text, purpose, hints) bridging
`TextInputClient`; `wp_fractional_scale` + `wp_viewporter`; `xdg-decoration` with our CSD fallback (hit-test
regions); subsurfaces for OS layers locked with the toplevel state (sync during resize, desync otherwise);
`wp_presentation` feedback and frame callbacks as the frame source, `fifo-v1`/`commit-timing-v1` where
present, the 100 ms occlusion rule; `xdg-toplevel-drag-v1` tab tear-off; data device (clipboard, primary
selection, DnD); cursor shape protocol; the desktop portal (ashpd) for theme and dialogs. Tests (written,
not run): key mapping, input frame assembly.
