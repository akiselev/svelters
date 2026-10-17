# FW1: WinUI on the Windows runner (lane notes)

Branch `ci/fix-winui` (from `ci/platforms`). Goal: WinUI init, `generated`, the counter-win
selftest and the counter test green on GitHub's windows-latest (Server 2025).

## Init model: islands (no `Application.Start`)

The host contract (`plans/architecture/backend-abi.md`) creates widgets before `run()`, on one
toolkit thread. `Application.Start` blocks and only allows XAML inside its callback, so it would
need the host to move its creation work into a callback, which turns the contract inside out.
`DesktopWindowXamlSource` is only needed if `Microsoft.UI.Xaml.Window` can't be created in
islands mode. The Windows App SDK's own pattern for "XAML without Application.Start" is the
islands setup, and it fits the contract as is. WindowsAppSDK-Samples
`Samples/Islands/cs-winforms-unpackaged` (`Program.cs`, `XamlApp.cs`) does:

1. `DispatcherQueueController.CreateOnCurrentThread()` on the STA thread;
2. `new XamlApp()`: an `Application` subclass that is also `IXamlMetadataProvider`
   (delegating to `XamlControlsXamlMetaDataProvider`); its constructor calls
   `WindowsXamlManager.InitializeForCurrentThread()`;
3. only then, XAML objects, including `XamlControlsResources` merged into `Resources`;
4. shutdown: close the app, then `controller.ShutdownQueue()`.

## Root causes (run 1 of CI1)

- **0x8001010E at `XamlControlsResources`.** WIN1's `Xaml::init` created `XamlControlsResources`
  *before* `WindowsXamlManager::InitializeForCurrentThread`. Before that call there is no XAML
  core on the thread, and XAML reports that as RPC_E_WRONG_THREAD ("marshalled for a different
  thread"). It is not a real cross-thread call: the tests were already on the main thread.
  Fix: the islands order above.
- **Refcount underflow in `Application::compose`.** The test harness retried `WinUiBackend::new()`
  for every test after the first one failed. The second attempt composed a second `Application` on
  a thread where XAML still held the first one's inner object (`Application.Current`). The factory
  refused it and over-released our outer object, so windows-core's `WeakRefCount::release`
  underflowed when `compose` dropped its outer reference. windows-rs's compose itself (outer +
  `ComposeBase` slot, the result discarded the way C++/WinRT does it) is consistent with the
  aggregation contract. Fix: init exactly once per thread (`ATTEMPTED` guard in `app.rs`); the
  harness caches the first error (`testing.rs`); on a mid-init failure the composed app is leaked,
  not torn down.

## Threading of the tests

The generated tests and the counter test are already `harness = false`, so they run one after
another on the process's main thread. That thread is the UI thread (`RoInitialize(STA)` in
`Xaml::init`), like the AppKit/GTK runners. The counter test spawns `counter-win --selftest`,
which owns its main thread.

## Diagnostics

- `SVELTERS_WINUI_TRACE=1` prints every init step (set in the workflow).
- The selftest writes `report.json` on init failure too.
- Test logs go to `logs/` and are uploaded with the report artifact.

## Run log
