# Lane states (POL1-STATES)

Branch `ci/pb1-states`; record `plans/milestones/POL1-STATES.md`. Read `BRIEF.md` first: its "How to work" is the method
and its table row for `states` is your scope and your files.

Your work list: the rows of `plans/milestones/GAL2-stories.md` "Polish findings" (and GAL1's and GAL3's styling notes)
that fall in your scope, then what you find yourself on your family's sheets. Record each as a row: finding, cause,
fix (file), before/after looked at (yes), snapshot tests moved.

Done looks like: every finding in your scope fixed and seen fixed on its sheet in all four themes at zoom 1 and 2, or
recorded with a reason and a recommendation; `skelter-gallery check` still passes; the six workflows green.

Start with one shared mechanism, not per kind: a focus ring drawn from `--sv-focus-border` (VS Code's
`focusBorder`) for `:focus-visible` on every focusable kind; a disabled look from the roles (VS Code dims
foreground to `disabledForeground`); hover and pressed backgrounds from the toolbar/list hover roles. Check the
high-contrast themes especially: in VS Code they use outlines (`contrastActiveBorder`, `contrastBorder`), not fills.
