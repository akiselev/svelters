# a1 gtk-bars / gtk-dialogs

- Popover popup/popdown tests need the popover parented to a widget inside a presented toplevel; otherwise SIGSEGV in gdk_surface_new_popup.
- Unexposing 12 kinds lowers the tagged entity count asserted in tools/docs/test_ir.py (101 -> 89).
- Window zero-arg methods other than close use expect true (no WM under xvfb, state props do not change).
