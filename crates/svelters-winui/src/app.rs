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
//!    `XamlControlsXamlMetaDataProvider`) and `XamlControlsResources` merged into its resources,
//!    as unpackaged C++ apps without markup do. Without the resources controls have no template.
//! 5. `WindowsXamlManager::InitializeForCurrentThread` (skipped with `SVELTERS_WINUI_XAML_MANAGER=0`).
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
    pub manager: Option<WindowsXamlManager>,
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

impl Xaml {
    /// Initializes WinUI on the calling thread. Call once per thread.
    pub fn init() -> Result<Xaml, String> {
        // S_FALSE (already initialized) is fine; a different apartment is not.
        unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }.ok().map_err(|e| format!("RoInitialize(single-threaded): {}", crate::conv::err(e)))?;
        start_runtime()?;
        let e = |what: &'static str| move |err: windows_core::Error| format!("{what}: {}", crate::conv::err(err));
        let controller = DispatcherQueueController::CreateOnCurrentThread().map_err(e("DispatcherQueueController::CreateOnCurrentThread"))?;
        let queue = controller.DispatcherQueue().map_err(e("DispatcherQueueController.DispatcherQueue"))?;
        let provider = XamlControlsXamlMetaDataProvider::new().map_err(e("XamlControlsXamlMetaDataProvider"))?;
        let app = Application::compose(App { provider }).map_err(e("Application (composed with IXamlMetadataProvider)"))?;
        let resources: ResourceDictionary = XamlControlsResources::new().map_err(e("XamlControlsResources"))?.cast().map_err(e("XamlControlsResources as ResourceDictionary"))?;
        app.Resources()
            .and_then(|r| r.MergedDictionaries())
            .and_then(|m| m.Append(&resources))
            .map_err(e("Application.Resources.MergedDictionaries.Append(XamlControlsResources)"))?;
        let manager = if std::env::var("SVELTERS_WINUI_XAML_MANAGER").as_deref() == Ok("0") {
            None
        } else {
            Some(WindowsXamlManager::InitializeForCurrentThread().map_err(e("WindowsXamlManager::InitializeForCurrentThread"))?)
        };
        let pretranslate = load("Microsoft.UI.Windowing.Core.dll")
            .ok()
            .and_then(|m| proc(m, b"ContentPreTranslateMessage\0"))
            .map(|f| unsafe { std::mem::transmute::<_, PreTranslate>(f) });
        if pretranslate.is_none() {
            eprintln!("[svelters-winui] ContentPreTranslateMessage not found: keyboard accelerators may not reach XAML");
        }
        Ok(Xaml { controller, queue, app, manager, pretranslate })
    }
}

impl Drop for Xaml {
    fn drop(&mut self) {
        if let Some(m) = self.manager.take() {
            let _ = m.Close();
        }
        let _ = self.controller.ShutdownQueue();
    }
}
