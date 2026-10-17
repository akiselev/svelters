# Windows setup: build, run and test the WinUI 3 backend

Status: 2026-10-17, lane WIN1. Everything below was written on Linux.

What was checked there:

- The crates cargo-check for `x86_64-pc-windows-gnu` and `x86_64-pc-windows-msvc`.
- The example links for both targets (MSVC through `cargo xwin`).
- The exe starts under Wine and gets as far as the Windows App SDK bootstrapper.

**Nothing WinUI has run on real Windows yet.** This file is the procedure for doing that.
Background and design: `docs/notes/win1-winui.md`.

## 1. Toolchain (once)

All commands are PowerShell, on Windows 10 1809+ or Windows 11, x64.

1. **Visual Studio 2022 Build Tools.** Install the workload "Desktop development with C++".
   - It brings MSVC v143, the linker and the Windows 11 SDK.
   - The linker is needed for the exe, and `cl.exe` compiles QuickJS (`rquickjs-sys`).
   - Command: `winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive"`
2. **Rust.** `winget install Rustlang.Rustup`, then open a new shell.
   - The repo pins 1.97.0 in `rust-toolchain.toml`, so the first `cargo` call installs it.
   - The default host target `x86_64-pc-windows-msvc` is the one to use.
3. **Node 22+ and pnpm 10.**
   - `winget install OpenJS.NodeJS.LTS`, then `corepack enable`.
   - pnpm then follows `packageManager: pnpm@10.33.0`.
4. **Python 3.11+** (`winget install Python.Python.3.12`). Only `fetch.py` needs it, to
   download the bootstrapper DLL; it uses the standard library only.
5. **The Svelte fork needs nothing extra.**
   - `pnpm install` resolves `svelte` to the vendored tarball (`vendor/svelte/*.tgz`, see
     `pnpm-workspace.yaml` overrides).
   - Make sure git checks the tarball out as binary (it's committed as such; don't enable
     `core.autocrlf` text conversion on `.tgz`).

## 2. The Windows App SDK runtime

The backend is **unpackaged and framework-dependent**:

- At startup it calls `MddBootstrapInitialize2(0x00020005, "", 2.5.1.0)` from
  `Microsoft.WindowsAppRuntime.Bootstrap.dll`.
- That adds the installed **Windows App Runtime 2.5** framework package to the process.
- The DLL comes from the NuGet package `Microsoft.WindowsAppSDK.Foundation` 2.3.12 and must be
  found next to the exe, in `SVELTERS_WINAPPSDK_DIR`, or in this checkout's `target/winsdk`
  (where step 3 puts it).

Install the runtime (2.5.x, x64) **one** of these ways:

- **A. Installer (simplest, URL unverified).**
  - Get "Windows App Runtime 2.5" from
    <https://learn.microsoft.com/windows/apps/windows-app-sdk/downloads> and run
    `WindowsAppRuntimeInstall-x64.exe`.
  - Or try `winget search WindowsAppRuntime` and install the 2.5 entry.
- **B. From the NuGet runtime package (contents verified on Linux).**

  ```powershell
  $v = "2.5.1"
  Invoke-WebRequest "https://api.nuget.org/v3-flatcontainer/microsoft.windowsappsdk.runtime/$v/microsoft.windowsappsdk.runtime.$v.nupkg" -OutFile rt.zip
  Expand-Archive rt.zip rt
  Get-ChildItem rt\tools\MSIX\win10-x64\*.msix | ForEach-Object { Add-AppxPackage $_.FullName }
  ```

  The four packages are `Microsoft.WindowsAppRuntime.2` (framework), `.Main.2`, `.Singleton` and
  `.DDLM.2`. If `Add-AppxPackage` refuses Main/Singleton, the framework package alone is often
  enough for the bootstrapper; report what happened.

Check it: `Get-AppxPackage *WindowsAppRuntime.2* | Select Name, Version`. Expect version 2.5.1.0
or later.

Other deployment modes are selected with `SVELTERS_WINAPPSDK`, but neither has been set up yet:

- `packaged`: running inside an MSIX that declares the dependency, so there's no bootstrapper.
- `self-contained`: the runtime DLLs ship next to the exe, and the backend calls
  `WindowsAppRuntime_EnsureIsLoaded`. That also needs the generated manifest and `.pri` files
  that the SDK's MSBuild targets write.

## 3. Build and run

```powershell
git clone <repo> svelters; cd svelters; git checkout <WIN1 branch or master after merge>
pnpm install
pnpm --filter ./fixtures/counter-win build          # -> fixtures/counter-win/dist/app.js
python tools/metadata/winui/fetch.py --bootstrap     # downloads ~110 MB of NuGet packages into target/winsdk, prints the DLL path

cargo test -p svelters-winui --lib                   # platform-neutral tables and parsers (same as Linux)
cargo test -p svelters-winui --test generated        # 60 generated WinUI verification tests, one STA thread
cargo run -p counter-win                             # the counter window; close it to exit
cargo run -p counter-win -- --selftest --report report.json
cargo test -p counter-win --test counter             # the same selftest as a test
```

- `--selftest` clicks the button three times through UI Automation (`ButtonAutomationPeer.Invoke`)
  and types `Ada` into the text box.
- It also sets the slider to 42 and checks the check box.
- It then reads the widgets back. It expects `Count: 3`, `Hello Ada` and
  `Level 42, agree: yes`.
- It prints a JSON report and exits 0 on pass.

Debug knobs (environment variables):

| Variable | Effect |
|---|---|
| `SVELTERS_WINAPPSDK=bootstrap\|self-contained\|packaged` | runtime mode (default `bootstrap`) |
| `SVELTERS_WINAPPSDK_DIR=<dir>` | where to find `Microsoft.WindowsAppRuntime.Bootstrap.dll` |
| `SVELTERS_WINAPPSDK_SHOW_UI=1` | the bootstrapper offers to install a missing runtime |
| `SVELTERS_WINUI_XAML_MANAGER=0` | skip `WindowsXamlManager.InitializeForCurrentThread` (try this if init fails after the Application step) |
| `SVELTERS_NO_MANIFEST=1` | build without the embedded app manifest (DPI awareness, `maxversiontested`) |

## 4. What to paste back

1. `winver`, or `[Environment]::OSVersion.Version`.
2. `Get-AppxPackage *WindowsAppRuntime* | Select Name, Version`.
3. The full output of `cargo test -p svelters-winui --test generated`. It prints one line per
   test and the failures with their messages.
4. The full output of `cargo run -p counter-win -- --selftest --report report.json`, and
   `report.json`.
5. Whether `cargo run -p counter-win` shows a styled window: a Fluent button, rounded corners
   and a slider. A screenshot helps.
6. On a crash with no output:
   - the exit code (`$LASTEXITCODE`);
   - the Event Viewer entry (Windows Logs, then Application, newest "Application Error" for
     `counter-win.exe`).
7. If init fails, the `[counter-win] WinUI init failed: ...` line. It names the step that failed
   (`DispatcherQueueController`, `Application`, `XamlControlsResources`,
   `WindowsXamlManager`, ...).

## 5. Not verified (checklist for the first Windows run)

- [ ] `MddBootstrapInitialize2` with the 2.5 constants finds the installed runtime. Under Wine it
      returns `E_NOTIMPL`, as expected.
- [ ] `Application::compose` with our `IXamlMetadataProvider` works outside `Application.Start`.
- [ ] `XamlControlsResources` loads, so controls get Fluent templates.
- [ ] `WindowsXamlManager::InitializeForCurrentThread` after the Application is harmless. If not,
      retry with `SVELTERS_WINUI_XAML_MANAGER=0`.
- [ ] **`Microsoft.UI.Xaml.Window` can be created and activated without `Application.Start`.**
      This is the biggest risk. The fallback is a `DesktopWindowXamlSource` in our own HWND
      (docs/notes/win1-winui.md).
- [ ] Our `GetMessage` loop pumps the DispatcherQueue. `wake` (`TryEnqueue`), `schedule_wake`
      (`DispatcherQueueTimer`) and `request_frame` (`CompositionTarget.Rendering`) should all
      fire.
- [ ] `ContentPreTranslateMessage` is exported by `Microsoft.UI.Windowing.Core.dll` (verified in
      the import lib) and is reachable once the framework package is loaded. Without it, the
      backend warns and tab and accelerator handling may be off.
- [ ] Event wiring:
  - `Click` through `ButtonAutomationPeer.Invoke`;
  - `TextChanged`, which is raised asynchronously;
  - `ValueChanged` and `Checked`/`Unchecked` from programmatic sets;
  - `BeforeTextChanging.Cancel` veto through `call_sync`;
  - `Closed.Handled` veto.
- [ ] Grid `row-definitions` / `column-definitions` and `Grid.Row`/`Grid.Column` child attributes
      place children.
- [ ] Window close without a JS listener reports `SVL_GONE_CLOSED`, and the host then quits the
      loop.
- [ ] The embedded manifest links with MSVC `link.exe`. It was checked with lld-link through
      cargo-xwin only.
- [ ] The UIA tree is sane in Accessibility Insights or Inspect.
