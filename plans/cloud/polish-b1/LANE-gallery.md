# Lane gallery (POL1-GALLERY)

Branch `ci/pb1-gallery`; record `plans/milestones/POL1-GALLERY.md`. Read `BRIEF.md` first: its "How to work" is the method
and its table row for `gallery` is your scope and your files.

Your work list: the rows of `plans/milestones/GAL2-stories.md` "Polish findings" (and GAL1's and GAL3's styling notes)
that fall in your scope, then what you find yourself on your family's sheets. Record each as a row: finding, cause,
fix (file), before/after looked at (yes), snapshot tests moved.

Done looks like: every finding in your scope fixed and seen fixed on its sheet in all four themes at zoom 1 and 2, or
recorded with a reason and a recommendation; `skelter-gallery check` still passes; the six workflows green.

Order: (1) fonts with symbol, Arabic and CJK coverage, deterministic across machines (prefer a bundled fallback set
with licences that allow it, recorded; else `--system-fonts`); (2) the icon registry (skelter-widgets' icons by key)
and `asset://` SVGs in the gallery host; push both at once, the other lanes wait on them; (3) sheet header escaping;
(4) `--diff`; (5) `Renderer::reload`; (6) the `window` story; (7) R1, R3, R4, Q1, each the smallest change, each with
its own test. Keep `check` passing on every story.
