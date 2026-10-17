# FA1: AppKit backend fixes from the first macOS CI runs

Branch `ci/fix-appkit` (from `ci/platforms`, merged with `origin/master`). Runner: GitHub macOS arm64
(macOS 26.6.2, default Xcode 26.6 / SDK 26.5; Xcode 26.0, 26.0.1, 26.1.1 also installed).

## Backend test failures: root causes and fixes

1. **`read_only_prop_is_refused`** ("cannot convert list to an object"). The read-only check did
   run before conversion, but it passed: NSView declares `subviews` `readwrite` (`@property(copy)`),
   so the table says `writable: true`, and the value then failed to box. The prop is the
   `subviews` slot's child storage, and a write would change children behind `insert`/`remove`.
   Fix: `set_prop` now validates everything before touching the value or the native object.
   Any prop whose KVC key is a slot's `list`/`get` (`subviews`, `arranged-subviews`,
   `content-view`) is refused as read-only and the error names the slot
   (`lookup::slot_owning_key`). Then come plain read-only props, then `live()`, then conversion.
2. **`request_frame_delivers_one_frame`** (0 frames). The display link was added with
   `addToRunLoop:forMode: NSString::from_str("kCFRunLoopCommonModes")`, an equal string but not the
   `NSRunLoopCommonModes` constant, and CADisplayLink then never ticks. CI probe (run 39080789144):
   CVDisplayLink 60 Hz on the runner's 1024-wide virtual display. View link with the look-alike
   common-modes string: **0 ticks / 300 ms**. The same link in default mode: 19. Window link,
   screen link, layer-backed view, and links after `finishLaunching` each got 18-19. So the
   display does tick on CI runners, and the fix is the real constant (`extern static
   NSRunLoopCommonModes`). Hardening: every request also arms the fallback timer as a 64 ms
   watchdog, because a link does not tick for a hidden or occluded window. `deliver_frame` pauses
   the link whichever source wins. A test asserts that a visible window's frames come from the
   link (`AppKitBackend::frame_sources`), that an ordered-out window still gets exactly one frame,
   and that the no-window timer path works.
3. **`no_leaked_handles`** cascade: backend cases run under `Harness::case_guard()` (a `guarded!`
   macro builds `CASES`). On panic it destroys every handle the case created. Passing cases are
   left alone, so their own leaks are still caught.

Same class of bug elsewhere: `insert` into a one-child slot unlinked the old child before `link`
validated the new one, so a refused insert (child already parented, cycle) left the table
disagreeing with the native tree. It now restores the old child on any failure (test
`refused_replace_keeps_the_old_child`). `call`/`invoke` already check arity and family before
converting. `listen` validates before mutating.

Runner observation: CF timers fire 10-60 ms late on the runner (a 16 ms timer at 27-76 ms). An
activity token or `finishLaunching` did not change that. Frame tests now wait with a deadline
instead of a fixed pump. `SVELTERS_APPKIT_TRACE_FRAMES=1` traces the frame path. The CI backend
step sets it.

## Metadata from Xcode (licence)

- `ingest.py` no longer needs `--sdk`. It uses `xcrun --sdk macosx --show-sdk-path` (honours
  `DEVELOPER_DIR`), the active toolchain's `libclang.dylib` (next to `xcrun --find clang`, or
  `--libclang`), and records `MacOSX<Version>` from `SDKSettings.json`.
- Apple-clang differences fixed: it prints macro-qualified types by macro name
  (`API_AVAILABLE NSColor *`), so the bare macro is now stripped too. Cursor kinds newer than the
  Python bindings map to `UNEXPOSED_ATTR`. Bindings 18.1.8 lacked `FLAG_ENUM` (every NS_OPTIONS
  became an enum), so CI pins `clang==21.1.7`.
- Result with Xcode 26.1.1 / macOS 26.1 SDK: **byte-identical** to the Linux/mirror extraction,
  for all 11 JSON files, generated `tables.rs` and `generated.rs`. No data differences.
- CI job `appkit-metadata` picks the installed Xcode whose SDK version matches `_enums.json`'s
  `sdk`. If none matches, it warns and uses the default. It runs ingest + `gen_backend.py`,
  fails on `git diff`, and uploads the regenerated files (`appkit-metadata` artifact). Moving to
  a newer SDK means changing nothing, letting the job fail, then committing the artifact.

## Runs (macos-appkit workflow)

| Run | Result |
|---|---|
| 39080162171 | read-only + leak fixed; frames: link created, never ticks |
| 39080356580 | probe 1: CVDisplayLink ticks; ingest: bindings too old (kind 431) |
| 39080614141 | probe 2 compile error; ingest ran: Apple clang spelling diff |
| 39080789144 | all green; probe 2 pinpoints the mode string; metadata identical |
| 39080956075 | all green with the real fix, probe removed |

About 9 job-minutes of macOS in total (5 appkit jobs, about 1.2-1.7 min each, plus 4 metadata
jobs, about 0.5 min each).

## Only verifiable on a real Mac

- Display-link frames on a real display (ProMotion, multiple screens, moving a window between
  screens: the link follows the view's screen).
- Watchdog behaviour when the window is minimised or occluded by other apps.
- Interactive behaviour that the runner can't show: clicks, menus, live resize.
