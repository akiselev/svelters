//! Process-level WinUI setup: the Windows App SDK runtime, the XAML thread and the Application.
//!
//! Order (see docs/notes/win1-winui.md):
//! 1. `RoInitialize(single-threaded)`: WinUI objects are ASTA, bound to this thread.
//! 2. The Windows App SDK runtime (`SVELTERS_WINAPPSDK`):
//!    - `bootstrap` (default, unpackaged + framework-dependent): `MddBootstrapInitialize2` from
//!      `Microsoft.WindowsAppRuntime.Bootstrap.dll`, found next to the exe or in
//!      `SVELTERS_WINAPPSDK_DIR`. Needs the Windows App Runtime 2.5 installed.
//!    - `self-contained`: the runtime DLLs ship next to the exe; `WindowsAppRuntime_EnsureIsLoaded`.
//!    - `packaged`: the package graph already has the framework; nothing to do.
//! 3. `DispatcherQueueController::CreateOnCurrentThread`: our Win32 loop pumps it.
//! 4. An `Application` composed with an `IXamlMetadataProvider` (delegating to
//!    `XamlControlsXamlMetaDataProvider`).
//! 5. `WindowsXamlManager::InitializeForCurrentThread`: creates the XAML core on this thread.
//! 6. `XamlControlsResources` merged into the Application's resources (needs step 5). Without
//!    them controls have no template.
//!
//! No `Application.Start`: the host creates widgets before `run()`, so this is the islands model
//! (docs/notes/fw1-winui-ci.md).
//!
//! The bootstrap and `ContentPreTranslateMessage` (Microsoft.UI.Windowing.Core.dll) are loaded
//! with `LoadLibraryW`, not linked: they live in the framework package, which is only on the DLL
//! search path after step 2, and a missing runtime must be a readable error, not a loader failure.
use crate::bindings::Microsoft::UI::Dispatching::{DispatcherQueue, DispatcherQueueController};
use crate::bindings::Microsoft::UI::Xaml::Application;
use crate::bindings::Microsoft::UI::Xaml::Controls::XamlControlsResources;
use crate::bindings::Microsoft::UI::Xaml::Hosting::WindowsXamlManager;
use crate::bindings::Microsoft::UI::Xaml::Markup::{IXamlMetadataProvider, IXamlMetadataProvider_Impl, IXamlType, XmlnsDefinition};
use crate::bindings::Microsoft::UI::Xaml::ResourceDictionary;
use crate::bindings::Microsoft::UI::Xaml::XamlTypeInfo::XamlControlsXamlMetaDataProvider;
use crate::bindings::Windows::Foundation::IClosable;
use crate::bindings::Windows::UI::Xaml::Interop::TypeName;
use crate::bindings::Windows::Win32::{GetModuleFileNameW, GetProcAddress, LoadLibraryW, MSG, RO_INIT_SINGLETHREADED, RoInitialize};
use std::path::PathBuf;
use windows_core::{HSTRING, Interface, PCSTR, PCWSTR, implement};

/// Windows App SDK 2.5.1 (`Microsoft.WindowsAppSDK.Runtime` 2.5.1 `WindowsAppSDK-VersionInfo.json`).
pub const WINAPPSDK_MAJOR_MINOR: u32 = 0x0002_0005;
pub const WINAPPSDK_MIN_RUNTIME: u64 = 0x0002_0005_0001_0000;
const MDD_ON_NO_MATCH_SHOW_UI: u32 = 0x0008;
const MDD_ON_PACKAGE_IDENTITY_NOOP: u32 = 0x0010;

type MddBootstrapInitialize2 = unsafe extern "system" fn(u32, PCWSTR, u64, u32) -> windows_core::HRESULT;
type EnsureIsLoaded = unsafe extern "system" fn() -> windows_core::HRESULT;
pub type PreTranslate = unsafe extern "system" fn(*const MSG) -> windows_core::BOOL;

#[implement(IXamlMetadataProvider)]
struct App {
    provider: XamlControlsXamlMetaDataProvider,
}

impl IXamlMetadataProvider_Impl for App_Impl {
    fn GetXamlType(&self, ty: &TypeName) -> windows_core::Result<IXamlType> {
        self.provider.GetXamlType(ty)
    }
    fn GetXamlTypeByFullName(&self, name: &HSTRING) -> windows_core::Result<IXamlType> {
        self.provider.GetXamlTypeByFullName(name)
    }
    fn GetXmlnsDefinitions(&self) -> windows_core::Result<windows_core::Array<XmlnsDefinition>> {
        self.provider.GetXmlnsDefinitions()
    }
}

/// Everything that must stay alive while XAML runs on this thread.
pub struct Xaml {
    pub controller: DispatcherQueueController,
    pub queue: DispatcherQueue,
    /// Kept alive: XAML resources hang off the Application for the lifetime of the thread.
    #[allow(dead_code)]
    pub app: Application,
    pub manager: std::cell::RefCell<Option<WindowsXamlManager>>,
    /// Set by [`Xaml::shutdown`].
    pub down: std::cell::Cell<bool>,
    pub pretranslate: Option<PreTranslate>,
}

fn exe_dir() -> Option<PathBuf> {
    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetModuleFileNameW(None, windows_core::PWSTR(buf.as_mut_ptr()), buf.len() as u32) } as usize;
    let p = PathBuf::from(String::from_utf16_lossy(&buf[..n]));
    p.parent().map(|d| d.to_path_buf())
}

fn load(path: &str) -> Result<crate::bindings::Windows::Win32::HMODULE, String> {
    let wide = HSTRING::from(path);
    let m = unsafe { LoadLibraryW(PCWSTR(wide.as_ptr())) };
    if m.0.is_null() {
        return Err(format!("LoadLibraryW({path}): {}", crate::conv::err(windows_core::Error::from_thread())));
    }
    Ok(m)
}

fn proc(module: crate::bindings::Windows::Win32::HMODULE, name: &[u8]) -> Option<unsafe extern "system" fn() -> isize> {
    unsafe { GetProcAddress(module, PCSTR(name.as_ptr())) }
}

fn runtime_dll(name: &str) -> String {
    // Dev fallback: where tools/metadata/winui/fetch.py unpacks the Foundation package in this checkout.
    let fetched = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/winsdk/microsoft.windowsappsdk.foundation/2.3.12/runtimes/win-x64/native");
    let dirs = [std::env::var("SVELTERS_WINAPPSDK_DIR").ok().map(PathBuf::from), exe_dir(), Some(fetched)];
    for d in dirs.into_iter().flatten() {
        let p = d.join(name);
        if p.is_file() {
            return p.display().to_string();
        }
    }
    name.to_string()
}

fn start_runtime() -> Result<(), String> {
    let mode = std::env::var("SVELTERS_WINAPPSDK").unwrap_or_else(|_| "bootstrap".into());
    match mode.as_str() {
        "packaged" => Ok(()),
        "self-contained" => {
            let m = load(&runtime_dll("Microsoft.WindowsAppRuntime.dll"))?;
            let f = proc(m, b"WindowsAppRuntime_EnsureIsLoaded\0").ok_or("Microsoft.WindowsAppRuntime.dll has no WindowsAppRuntime_EnsureIsLoaded")?;
            let f: EnsureIsLoaded = unsafe { std::mem::transmute(f) };
            unsafe { f() }.ok().map_err(|e| format!("WindowsAppRuntime_EnsureIsLoaded: {}", crate::conv::err(e)))
        }
        "bootstrap" => {
            let dll = runtime_dll("Microsoft.WindowsAppRuntime.Bootstrap.dll");
            let m = load(&dll).map_err(|e| {
                format!(
                    "{e}\nThe Windows App SDK bootstrapper is missing: copy Microsoft.WindowsAppRuntime.Bootstrap.dll next to the exe \
                     or set SVELTERS_WINAPPSDK_DIR (python tools/metadata/winui/fetch.py --bootstrap prints its path). \
                     See docs/notes/windows-setup.md."
                )
            })?;
            let f = proc(m, b"MddBootstrapInitialize2\0").ok_or("bootstrap DLL has no MddBootstrapInitialize2")?;
            let f: MddBootstrapInitialize2 = unsafe { std::mem::transmute(f) };
            let tag = HSTRING::new();
            let options = MDD_ON_PACKAGE_IDENTITY_NOOP
                | if std::env::var_os("SVELTERS_WINAPPSDK_SHOW_UI").is_some() { MDD_ON_NO_MATCH_SHOW_UI } else { 0 };
            unsafe { f(WINAPPSDK_MAJOR_MINOR, PCWSTR(tag.as_ptr()), WINAPPSDK_MIN_RUNTIME, options) }.ok().map_err(|e| {
                format!(
                    "MddBootstrapInitialize2(2.5, min 2.5.1.0): {}\nIs the Windows App Runtime 2.5 installed? See docs/notes/windows-setup.md.",
                    crate::conv::err(e)
                )
            })
        }
        other => Err(format!("SVELTERS_WINAPPSDK={other}: expected bootstrap, self-contained or packaged")),
    }
}

thread_local! {
    /// Set by the first [`Xaml::init`] on a thread, success or not. WinUI keeps per-thread state
    /// (the XAML core, `Application.Current`) that a failed init does not undo, and a second
    /// composed `Application` on the same thread over-released our outer object in CI1
    /// (`weak_ref_count.rs` subtract with overflow inside `Application::compose`). So: once.
    static ATTEMPTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `SVELTERS_WINUI_TRACE=1`: print each init step to stderr (CI diagnostics).
fn trace(step: &str) {
    if std::env::var_os("SVELTERS_WINUI_TRACE").is_some_and(|v| v != "0") {
        eprintln!("[svelters-winui] init: {step}");
    }
}

impl Xaml {
    /// Initializes WinUI on the calling thread, which becomes the UI thread. Once per thread.
    ///
    /// The order is the Windows App SDK islands pattern (WindowsAppSDK-Samples
    /// `Samples/Islands/cs-winforms-unpackaged`, `Program.cs` + `XamlApp.cs`): dispatcher queue,
    /// then the `Application` (composed with our metadata provider), then
    /// `WindowsXamlManager::InitializeForCurrentThread`, which creates the XAML core for this
    /// thread. Only after that can XAML objects be made: `XamlControlsResources` before it fails
    /// with RPC_E_WRONG_THREAD (0x8001010E), XAML's "no core on this thread" error (CI1).
    pub fn init() -> Result<Xaml, String> {
        if ATTEMPTED.with(|a| a.replace(true)) {
            return Err("WinUI was already initialized (or failed to) on this thread; it is set up once per thread".into());
        }
        trace(&format!("thread {:?}", std::thread::current().id()));
        // S_FALSE (already initialized) is fine; a different apartment is not.
        unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }.ok().map_err(|e| format!("RoInitialize(single-threaded): {}", crate::conv::err(e)))?;
        trace("RoInitialize(single-threaded) ok");
        start_runtime()?;
        trace("Windows App SDK runtime ok");
        let e = |what: &'static str| move |err: windows_core::Error| format!("{what}: {}", crate::conv::err(err));
        let controller = DispatcherQueueController::CreateOnCurrentThread().map_err(e("DispatcherQueueController::CreateOnCurrentThread"))?;
        let queue = controller.DispatcherQueue().map_err(e("DispatcherQueueController.DispatcherQueue"))?;
        trace("DispatcherQueueController ok");
        let provider = XamlControlsXamlMetaDataProvider::new().map_err(e("XamlControlsXamlMetaDataProvider"))?;
        let app = Application::compose(App { provider }).map_err(e("Application (composed with IXamlMetadataProvider)"))?;
        trace("Application composed");
        // From here on a failure leaks `app` (and the manager): tearing down a composed
        // Application whose XAML side is half set up is the path that over-released in CI1, and
        // the thread cannot be initialized again anyway.
        let manager = match WindowsXamlManager::InitializeForCurrentThread() {
            Ok(m) => m,
            Err(err) => {
                std::mem::forget(app);
                return Err(e("WindowsXamlManager::InitializeForCurrentThread")(err));
            }
        };
        trace("WindowsXamlManager::InitializeForCurrentThread ok");
        let merged = XamlControlsResources::new()
            .map_err(e("XamlControlsResources"))
            .and_then(|r| r.cast::<ResourceDictionary>().map_err(e("XamlControlsResources as ResourceDictionary")))
            .and_then(|r| {
                app.Resources()
                    .and_then(|d| d.MergedDictionaries())
                    .and_then(|m| m.Append(&r))
                    .map_err(e("Application.Resources.MergedDictionaries.Append(XamlControlsResources)"))
            });
        if let Err(err) = merged {
            std::mem::forget(app);
            std::mem::forget(manager);
            return Err(err);
        }
        trace("XamlControlsResources merged");
        let manager = Some(manager);
        let pretranslate = load("Microsoft.UI.Windowing.Core.dll")
            .ok()
            .and_then(|m| proc(m, b"ContentPreTranslateMessage\0"))
            .map(|f| unsafe { std::mem::transmute::<_, PreTranslate>(f) });
        if pretranslate.is_none() {
            eprintln!("[svelters-winui] ContentPreTranslateMessage not found: keyboard accelerators may not reach XAML");
        }
        Ok(Xaml { controller, queue, app, manager: std::cell::RefCell::new(manager), pretranslate, down: std::cell::Cell::new(false) })
    }

    /// The islands samples' shutdown: close the Application (it holds XAML resources), then the
    /// XAML manager, then shut the dispatcher queue down. Idempotent. Windows must be closed and
    /// every XAML object released before this (`WinUiBackend::shutdown`).
    pub fn shutdown(&self) {
        if self.down.replace(true) {
            return;
        }
        trace("shutdown: Application.Close");
        if let Ok(c) = self.app.cast::<IClosable>() {
            if let Err(e) = c.Close() {
                trace(&format!("Application.Close: {}", crate::conv::err(e)));
            }
        }
        trace("shutdown: WindowsXamlManager.Close");
        if let Some(m) = self.manager.borrow_mut().take() {
            let _ = m.Close();
        }
        trace("shutdown: DispatcherQueueController.ShutdownQueue");
        if let Err(e) = self.controller.ShutdownQueue() {
            trace(&format!("ShutdownQueue: {}", crate::conv::err(e)));
        }
        trace("shutdown: done");
    }
}

impl Drop for Xaml {
    fn drop(&mut self) {
        self.shutdown();
    }
}
