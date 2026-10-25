# Lane MACOS: presto-platform-macos

Owns `crates/presto-platform-macos/**`. objc2 (objc2-app-kit, objc2-quartz-core, objc2-foundation). Deliver:
NSApplication and our own NSWindow/NSView subclass; the full `NSTextInputClient` (marked text, attributed
substrings, `firstRectForCharacterRange` per range, `characterIndexForPoint`, `replacementRange`, selected
range, unmarkText) bridging `TextInputClient` — dictation, the Character Viewer, press-and-hold accents and
hanja must work; keyboard via `UCKeyTranslate` with egui key names and the Option-letter policy; mouse with
coalescing control and tablet/pen events; precise scrolling with `phase`/`momentumPhase`; magnify/rotate
gestures; CAMetalLayer per window plus sublayers for OS layers; `CAMetalDisplayLink` (macOS 14+, fallback
`NSView.displayLink`) as the frame source with `targetTimestamp`; live-resize with `presentsWithTransaction`
(the main thread presents); the app delegate (Cmd-Q veto, reopen); an AccessKit host view (no subclassing
adapter); muda for the menu bar; cursors; clipboard. Tests (written, not run): key mapping, the text-input
range logic.
