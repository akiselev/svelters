# Lane WIN32: presto-platform-win32

Owns `crates/presto-platform-win32/**`. The `windows` crate. Deliver: our own window class and procedure;
the modal-loop policy (rendering and timers continue through move/size/menu loops; frames come from the
compositor's wake); `WM_POINTER` for mouse, pen and touch with `GetPointerInfoHistory` and QPC timestamps;
keyboard (scancodes, `ToUnicodeEx`, dead keys, AltGr apart from Ctrl+Alt, egui key names); a TSF text store
(ITextStoreACP) bridging `TextInputClient` (UTF-16 offsets), with `WM_IME_REQUEST` reconversion for
ATOK/Mozc; DirectManipulation for precision touchpad pan/pinch/inertia (port GPUI `direct_manipulation.rs`);
DirectComposition device and tree (window root visual, child visuals for OS layers, `SurfaceTarget::
CompositionVisual`); `WM_NCHITTEST` hit-test regions with snap layouts (`HTMAXBUTTON`); per-monitor DPI v2;
an OLE drag source that keeps dispatch running; clipboard; cursors; the frame source (DXGI waitable /
`DCompositionWaitForCompositorClock` with boost). Tests (written, not run): message translation, key names,
TSF range mapping.
