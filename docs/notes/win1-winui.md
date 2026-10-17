# WIN1: WinUI 3 backend (lane notes)

Status: in progress, 2026-10-17. Written on Linux. Builds on `docs/notes/windows-gui-backend.md`.

## Decisions so far

- **Hosting: XAML Islands-style init, not `Application.Start`.**
  - The host loads the bundle, and so calls `create`, *before* `backend.run()`
    (`examples/counter/src/main.rs`: `Host::new` -> `load_bundle` -> `host.run()`).
  - `Application.Start` blocks and only lets us create XAML objects inside its callback, so it
    doesn't fit.
  - The backend therefore initializes XAML in `new()`:
    `DispatcherQueueController::CreateOnCurrentThread`, then
    `WindowsXamlManager::InitializeForCurrentThread`, then an `Application` instance (needed
    for `XamlControlsResources`).
  - `run()` is our own `GetMessage` loop calling `ContentPreTranslateMessage`.
  - This matches the research recommendation.
- **`window` is `Microsoft.UI.Xaml.Window`** (tag rule: native class name), not our own HWND +
  `DesktopWindowXamlSource`.
  - Risk: it's unverified whether `Window` can be constructed outside `Application.Start` (see
    the checklist in `docs/notes/windows-setup.md`).
  - Fallback: a `DesktopWindowXamlSource` in our own HWND. The kind would stay `window`.
- **Windows App SDK version: 2.5.1** (metapackage), with WinUI 2.3.9, Foundation 2.3.12 and
  InteractiveExperiences 2.1.9.
  - Bootstrap constants come from `Microsoft.WindowsAppSDK.Runtime` 2.5.1
    `WindowsAppSDK-VersionInfo.json` **[verified]**:
    - major.minor `0x00020005`;
    - version tag `""`;
    - min runtime version `0x0002000500010000` (2.5.1.0).
  - Framework package family: `Microsoft.WindowsAppRuntime.2_8wekyb3d8bbwe`.
  - The Runtime nupkg (170 MB) carries the four runtime MSIX files
    (`tools/MSIX/win10-x64/*.msix`), installable with `Add-AppxPackage`.
