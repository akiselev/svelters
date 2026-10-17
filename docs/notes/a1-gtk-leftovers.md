# a1-gtk-leftovers

- 23 kinds marked unexposed (models -> virtuallist/G6, deprecated 4.10/4.18, internal).
- ColorDialogButton.rgba verified. FontDialogButton use-font/use-size/level verified.
- FontDialogButton.font-desc dropped: generated boxed round-trip compares boxed handles; each read mints a new handle (Boxed{handle:3} != Boxed{handle:2}).
