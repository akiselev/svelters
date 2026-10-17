pub mod Microsoft {
    pub mod UI {
        pub mod Dispatching {
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct DispatcherQueue(windows_core::IUnknown);
            windows_core::imp::interface_hierarchy!(
                DispatcherQueue,
                windows_core::IUnknown,
                windows_core::IInspectable
            );
            impl DispatcherQueue {
                pub fn CreateTimer(&self) -> windows_core::Result<DispatcherQueueTimer> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).CreateTimer)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .and_then(|| windows_core::imp::Type::from_abi(result__))
                    }
                }
                pub fn TryEnqueue<P0>(&self, callback: P0) -> windows_core::Result<bool>
                where
                    P0: windows_core::Param<DispatcherQueueHandler>,
                {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).TryEnqueue)(
                            windows_core::Interface::as_raw(self),
                            callback.param().abi(),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                fn IDispatcherQueueStatics<
                    R,
                    F: FnOnce(&IDispatcherQueueStatics) -> windows_core::Result<R>,
                >(
                    callback: F,
                ) -> windows_core::Result<R> {
                    static SHARED: windows_core::imp::FactoryCache<
                        DispatcherQueue,
                        IDispatcherQueueStatics,
                    > = windows_core::imp::FactoryCache::new();
                    SHARED.call(callback)
                }
            }
            impl windows_core::RuntimeType for DispatcherQueue {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_class::<Self, IDispatcherQueue>();
            }
            unsafe impl windows_core::Interface for DispatcherQueue {
                type Vtable = <IDispatcherQueue as windows_core::Interface>::Vtable;
                const IID: windows_core::GUID = <IDispatcherQueue as windows_core::Interface>::IID;
            }
            impl windows_core::RuntimeName for DispatcherQueue {
                const NAME: &'static str = "Microsoft.UI.Dispatching.DispatcherQueue";
            }
            unsafe impl Send for DispatcherQueue {}
            unsafe impl Sync for DispatcherQueue {}
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct DispatcherQueueController(windows_core::IUnknown);
            windows_core::imp::interface_hierarchy!(
                DispatcherQueueController,
                windows_core::IUnknown,
                windows_core::IInspectable
            );
            impl DispatcherQueueController {
                pub fn DispatcherQueue(&self) -> windows_core::Result<DispatcherQueue> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).DispatcherQueue)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .and_then(|| windows_core::imp::Type::from_abi(result__))
                    }
                }
                pub fn ShutdownQueue(&self) -> windows_core::Result<()> {
                    let this = &windows_core::Interface::cast::<IDispatcherQueueController2>(self)?;
                    unsafe {
                        (windows_core::Interface::vtable(this).ShutdownQueue)(
                            windows_core::Interface::as_raw(this),
                        )
                        .ok()
                    }
                }
                pub fn CreateOnCurrentThread() -> windows_core::Result<Self> {
                    Self::IDispatcherQueueControllerStatics(|this| unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(this).CreateOnCurrentThread)(
                            windows_core::Interface::as_raw(this),
                            &mut result__,
                        )
                        .and_then(|| windows_core::imp::Type::from_abi(result__))
                    })
                }
                fn IDispatcherQueueControllerStatics<
                    R,
                    F: FnOnce(&IDispatcherQueueControllerStatics) -> windows_core::Result<R>,
                >(
                    callback: F,
                ) -> windows_core::Result<R> {
                    static SHARED: windows_core::imp::FactoryCache<
                        DispatcherQueueController,
                        IDispatcherQueueControllerStatics,
                    > = windows_core::imp::FactoryCache::new();
                    SHARED.call(callback)
                }
            }
            impl windows_core::RuntimeType for DispatcherQueueController {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_class::<Self, IDispatcherQueueController>();
            }
            unsafe impl windows_core::Interface for DispatcherQueueController {
                type Vtable = <IDispatcherQueueController as windows_core::Interface>::Vtable;
                const IID: windows_core::GUID =
                    <IDispatcherQueueController as windows_core::Interface>::IID;
            }
            impl windows_core::RuntimeName for DispatcherQueueController {
                const NAME: &'static str = "Microsoft.UI.Dispatching.DispatcherQueueController";
            }
            unsafe impl Send for DispatcherQueueController {}
            unsafe impl Sync for DispatcherQueueController {}
            windows_core::imp::define_interface!(
                DispatcherQueueHandler,
                DispatcherQueueHandler_Vtbl,
                0x2e0872a9_4e29_5f14_b688_fb96d5f9d5f8
            );
            impl windows_core::RuntimeType for DispatcherQueueHandler {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
            }
            impl DispatcherQueueHandler {
                pub fn new<F: Fn() -> windows_core::Result<()> + Send + 'static>(
                    invoke: F,
                ) -> Self {
                    let com = windows_core::imp::DelegateBox::<Self, F>::new(
                        &DispatcherQueueHandlerBox::<F>::VTABLE,
                        invoke,
                    );
                    unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
                }
                pub fn Invoke(&self) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).Invoke)(
                            windows_core::Interface::as_raw(self),
                        )
                        .ok()
                    }
                }
            }
            #[repr(C)]
            pub struct DispatcherQueueHandler_Vtbl {
                base__: windows_core::IUnknown_Vtbl,
                Invoke: unsafe extern "system" fn(
                    this: *mut core::ffi::c_void,
                ) -> windows_core::HRESULT,
            }
            struct DispatcherQueueHandlerBox<F: Fn() -> windows_core::Result<()> + Send + 'static>(
                core::marker::PhantomData<(fn() -> F,)>,
            );
            impl<F: Fn() -> windows_core::Result<()> + Send + 'static> DispatcherQueueHandlerBox<F> {
                const VTABLE: DispatcherQueueHandler_Vtbl = DispatcherQueueHandler_Vtbl {
                    base__:
                        windows_core::IUnknown_Vtbl {
                            QueryInterface: windows_core::imp::DelegateBox::<
                                DispatcherQueueHandler,
                                F,
                            >::QueryInterface,
                            AddRef:
                                windows_core::imp::DelegateBox::<DispatcherQueueHandler, F>::AddRef,
                            Release:
                                windows_core::imp::DelegateBox::<DispatcherQueueHandler, F>::Release,
                        },
                    Invoke: Self::Invoke,
                };
                unsafe extern "system" fn Invoke(
                    this: *mut core::ffi::c_void,
                ) -> windows_core::HRESULT {
                    unsafe {
                        let this = &mut *(this as *mut *mut core::ffi::c_void
                            as *mut windows_core::imp::DelegateBox<DispatcherQueueHandler, F>);
                        (this.invoke)().into()
                    }
                }
            }
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct DispatcherQueueTimer(windows_core::IUnknown);
            windows_core::imp::interface_hierarchy!(
                DispatcherQueueTimer,
                windows_core::IUnknown,
                windows_core::IInspectable
            );
            impl DispatcherQueueTimer {
                pub fn SetInterval(
                    &self,
                    value: windows_time::TimeSpan,
                ) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetInterval)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn SetIsRepeating(&self, value: bool) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetIsRepeating)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn Start(&self) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).Start)(
                            windows_core::Interface::as_raw(self),
                        )
                        .ok()
                    }
                }
                pub fn Stop(&self) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).Stop)(
                            windows_core::Interface::as_raw(self),
                        )
                        .ok()
                    }
                }
                pub fn Tick<F>(
                    &self,
                    handler: F,
                ) -> windows_core::Result<windows_core::EventRevoker>
                where
                    F: Fn(windows_core::Ref<Self>, windows_core::Ref<windows_core::IInspectable>)
                        + Send
                        + 'static,
                {
                    let handler = <super::super::super::Windows::Foundation::TypedEventHandler<
                        Self,
                        windows_core::IInspectable,
                    >>::new(move |a0, a1| {
                        handler(a0, a1);
                        Ok(())
                    });
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        let token__ = (windows_core::Interface::vtable(self).Tick)(
                            windows_core::Interface::as_raw(self),
                            windows_core::Interface::as_raw(&handler),
                            &mut result__,
                        )
                        .map(|| result__)?;
                        Ok(windows_core::EventRevoker::new(
                            self.clone(),
                            token__,
                            windows_core::Interface::vtable(self).RemoveTick,
                        ))
                    }
                }
            }
            impl windows_core::RuntimeType for DispatcherQueueTimer {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_class::<Self, IDispatcherQueueTimer>();
            }
            unsafe impl windows_core::Interface for DispatcherQueueTimer {
                type Vtable = <IDispatcherQueueTimer as windows_core::Interface>::Vtable;
                const IID: windows_core::GUID =
                    <IDispatcherQueueTimer as windows_core::Interface>::IID;
            }
            impl windows_core::RuntimeName for DispatcherQueueTimer {
                const NAME: &'static str = "Microsoft.UI.Dispatching.DispatcherQueueTimer";
            }
            unsafe impl Send for DispatcherQueueTimer {}
            unsafe impl Sync for DispatcherQueueTimer {}
            windows_core::imp::define_interface!(
                IDispatcherQueue,
                IDispatcherQueue_Vtbl,
                0xf6ebf8fa_be1c_5bf6_a467_73da28738ae8
            );
            impl windows_core::RuntimeType for IDispatcherQueue {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Dispatching.IDispatcherQueue",
                    );
            }
            #[repr(C)]
            pub struct IDispatcherQueue_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                pub CreateTimer: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                )
                    -> windows_core::HRESULT,
                pub TryEnqueue: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                    *mut bool,
                ) -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IDispatcherQueueController,
                IDispatcherQueueController_Vtbl,
                0xbce8178d_2183_584c_9e5b_f9366f6ae484
            );
            impl windows_core::RuntimeType for IDispatcherQueueController {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Dispatching.IDispatcherQueueController",
                    );
            }
            #[repr(C)]
            pub struct IDispatcherQueueController_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                pub DispatcherQueue: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                )
                    -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IDispatcherQueueController2,
                IDispatcherQueueController2_Vtbl,
                0x4c68ee2a_1cb1_5591_a3a2_9b590b8f8b9a
            );
            impl windows_core::RuntimeType for IDispatcherQueueController2 {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Dispatching.IDispatcherQueueController2",
                    );
            }
            #[repr(C)]
            pub struct IDispatcherQueueController2_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                pub ShutdownQueue:
                    unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IDispatcherQueueControllerStatics,
                IDispatcherQueueControllerStatics_Vtbl,
                0xf18d6145_722b_593d_bcf2_a61e713f0037
            );
            impl windows_core::RuntimeType for IDispatcherQueueControllerStatics {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Dispatching.IDispatcherQueueControllerStatics",
                    );
            }
            #[repr(C)]
            pub struct IDispatcherQueueControllerStatics_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                CreateOnDedicatedThread: usize,
                pub CreateOnCurrentThread: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                )
                    -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IDispatcherQueueStatics,
                IDispatcherQueueStatics_Vtbl,
                0xcd3382ea_a455_5124_b63a_ca40d34ca23c
            );
            impl windows_core::RuntimeType for IDispatcherQueueStatics {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Dispatching.IDispatcherQueueStatics",
                    );
            }
            #[repr(C)]
            pub struct IDispatcherQueueStatics_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
            }
            windows_core::imp::define_interface!(
                IDispatcherQueueTimer,
                IDispatcherQueueTimer_Vtbl,
                0xad4d63fd_88fe_541f_ac11_bf2dc1ed2ce5
            );
            impl windows_core::RuntimeType for IDispatcherQueueTimer {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Dispatching.IDispatcherQueueTimer",
                    );
            }
            #[repr(C)]
            pub struct IDispatcherQueueTimer_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                Interval: usize,
                pub SetInterval: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    windows_time::TimeSpan,
                )
                    -> windows_core::HRESULT,
                IsRunning: usize,
                IsRepeating: usize,
                pub SetIsRepeating: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    bool,
                )
                    -> windows_core::HRESULT,
                pub Start:
                    unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
                pub Stop:
                    unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
                pub Tick: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                    *mut i64,
                ) -> windows_core::HRESULT,
                pub RemoveTick:
                    unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
            }
        }
        pub mod Xaml {
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct Application(windows_core::IUnknown);
            windows_core::imp::interface_hierarchy!(
                Application,
                windows_core::IUnknown,
                windows_core::IInspectable
            );
            impl Application {
                pub fn Resources(&self) -> windows_core::Result<ResourceDictionary> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).Resources)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .and_then(|| windows_core::imp::Type::from_abi(result__))
                    }
                }
                pub fn new() -> windows_core::Result<Self> {
                    Self::IApplicationFactory(|this| unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(this).CreateInstance)(
                            windows_core::Interface::as_raw(this),
                            core::ptr::null_mut(),
                            core::ptr::null_mut(),
                            &mut result__,
                        )
                        .and_then(|| windows_core::imp::Type::from_abi(result__))
                    })
                }
                pub fn compose<T>(compose: T) -> windows_core::Result<Self>
                where
                    T: windows_core::Compose,
                {
                    Self::IApplicationFactory(|this| unsafe {
                        let (derived__, base__) = windows_core::Compose::compose(compose);
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(this).CreateInstance)(
                            windows_core::Interface::as_raw(this),
                            core::mem::transmute_copy(&derived__),
                            base__ as *mut _ as _,
                            &mut result__,
                        )
                        .ok()?;
                        let _ = &derived__;
                        windows_core::imp::Type::from_abi(result__)
                    })
                }
                fn IApplicationFactory<
                    R,
                    F: FnOnce(&IApplicationFactory) -> windows_core::Result<R>,
                >(
                    callback: F,
                ) -> windows_core::Result<R> {
                    static SHARED: windows_core::imp::FactoryCache<
                        Application,
                        IApplicationFactory,
                    > = windows_core::imp::FactoryCache::new();
                    SHARED.call(callback)
                }
                fn IApplicationStatics<
                    R,
                    F: FnOnce(&IApplicationStatics) -> windows_core::Result<R>,
                >(
                    callback: F,
                ) -> windows_core::Result<R> {
                    static SHARED: windows_core::imp::FactoryCache<
                        Application,
                        IApplicationStatics,
                    > = windows_core::imp::FactoryCache::new();
                    SHARED.call(callback)
                }
            }
            impl windows_core::RuntimeType for Application {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_class::<Self, IApplication>();
            }
            unsafe impl windows_core::Interface for Application {
                type Vtable = <IApplication as windows_core::Interface>::Vtable;
                const IID: windows_core::GUID = <IApplication as windows_core::Interface>::IID;
            }
            impl windows_core::RuntimeName for Application {
                const NAME: &'static str = "Microsoft.UI.Xaml.Application";
            }
            unsafe impl Send for Application {}
            unsafe impl Sync for Application {}
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct DependencyObject(windows_core::IUnknown);
            windows_core::imp::interface_hierarchy!(
                DependencyObject,
                windows_core::IUnknown,
                windows_core::IInspectable
            );
            impl DependencyObject {
                fn IDependencyObjectFactory<
                    R,
                    F: FnOnce(&IDependencyObjectFactory) -> windows_core::Result<R>,
                >(
                    callback: F,
                ) -> windows_core::Result<R> {
                    static SHARED: windows_core::imp::FactoryCache<
                        DependencyObject,
                        IDependencyObjectFactory,
                    > = windows_core::imp::FactoryCache::new();
                    SHARED.call(callback)
                }
            }
            impl windows_core::RuntimeType for DependencyObject {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_class::<Self, IDependencyObject>();
            }
            unsafe impl windows_core::Interface for DependencyObject {
                type Vtable = <IDependencyObject as windows_core::Interface>::Vtable;
                const IID: windows_core::GUID = <IDependencyObject as windows_core::Interface>::IID;
            }
            impl windows_core::RuntimeName for DependencyObject {
                const NAME: &'static str = "Microsoft.UI.Xaml.DependencyObject";
            }
            unsafe impl Send for DependencyObject {}
            unsafe impl Sync for DependencyObject {}
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct FrameworkElement(windows_core::IUnknown);
            windows_core::imp::interface_hierarchy!(
                FrameworkElement,
                windows_core::IUnknown,
                windows_core::IInspectable
            );
            windows_core::imp::required_hierarchy!(FrameworkElement, UIElement, DependencyObject);
            impl FrameworkElement {
                pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).ActualWidth)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).ActualHeight)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn Width(&self) -> windows_core::Result<f64> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).Width)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetWidth)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn Height(&self) -> windows_core::Result<f64> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).Height)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetHeight)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn MinWidth(&self) -> windows_core::Result<f64> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).MinWidth)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetMinWidth)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn MinHeight(&self) -> windows_core::Result<f64> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).MinHeight)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetMinHeight)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn HorizontalAlignment(&self) -> windows_core::Result<HorizontalAlignment> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).HorizontalAlignment)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetHorizontalAlignment(
                    &self,
                    value: HorizontalAlignment,
                ) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetHorizontalAlignment)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn VerticalAlignment(&self) -> windows_core::Result<VerticalAlignment> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).VerticalAlignment)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetVerticalAlignment(
                    &self,
                    value: VerticalAlignment,
                ) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetVerticalAlignment)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn Margin(&self) -> windows_core::Result<Thickness> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).Margin)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetMargin(&self, value: Thickness) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetMargin)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).Name)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| core::mem::transmute(result__))
                    }
                }
                pub fn SetName(&self, value: &windows_core::HSTRING) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetName)(
                            windows_core::Interface::as_raw(self),
                            core::mem::transmute_copy(value),
                        )
                        .ok()
                    }
                }
                pub fn Opacity(&self) -> windows_core::Result<f64> {
                    let this = &windows_core::Interface::cast::<IUIElement>(self)?;
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(this).Opacity)(
                            windows_core::Interface::as_raw(this),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                    let this = &windows_core::Interface::cast::<IUIElement>(self)?;
                    unsafe {
                        (windows_core::Interface::vtable(this).SetOpacity)(
                            windows_core::Interface::as_raw(this),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                    let this = &windows_core::Interface::cast::<IUIElement>(self)?;
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(this).IsHitTestVisible)(
                            windows_core::Interface::as_raw(this),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                    let this = &windows_core::Interface::cast::<IUIElement>(self)?;
                    unsafe {
                        (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                            windows_core::Interface::as_raw(this),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn Visibility(&self) -> windows_core::Result<Visibility> {
                    let this = &windows_core::Interface::cast::<IUIElement>(self)?;
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(this).Visibility)(
                            windows_core::Interface::as_raw(this),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetVisibility(&self, value: Visibility) -> windows_core::Result<()> {
                    let this = &windows_core::Interface::cast::<IUIElement>(self)?;
                    unsafe {
                        (windows_core::Interface::vtable(this).SetVisibility)(
                            windows_core::Interface::as_raw(this),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn KeyDown<F>(
                    &self,
                    handler: F,
                ) -> windows_core::Result<windows_core::EventRevoker>
                where
                    F: Fn(
                            windows_core::Ref<windows_core::IInspectable>,
                            windows_core::Ref<Input::KeyRoutedEventArgs>,
                        ) + Send
                        + 'static,
                {
                    let this = &windows_core::Interface::cast::<IUIElement>(self)?;
                    let handler = <Input::KeyEventHandler>::new(move |a0, a1| {
                        handler(a0, a1);
                        Ok(())
                    });
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                            windows_core::Interface::as_raw(this),
                            windows_core::Interface::as_raw(&handler),
                            &mut result__,
                        )
                        .map(|| result__)?;
                        Ok(windows_core::EventRevoker::new(
                            this.clone(),
                            token__,
                            windows_core::Interface::vtable(this).RemoveKeyDown,
                        ))
                    }
                }
                fn IFrameworkElementFactory<
                    R,
                    F: FnOnce(&IFrameworkElementFactory) -> windows_core::Result<R>,
                >(
                    callback: F,
                ) -> windows_core::Result<R> {
                    static SHARED: windows_core::imp::FactoryCache<
                        FrameworkElement,
                        IFrameworkElementFactory,
                    > = windows_core::imp::FactoryCache::new();
                    SHARED.call(callback)
                }
                fn IFrameworkElementStatics<
                    R,
                    F: FnOnce(&IFrameworkElementStatics) -> windows_core::Result<R>,
                >(
                    callback: F,
                ) -> windows_core::Result<R> {
                    static SHARED: windows_core::imp::FactoryCache<
                        FrameworkElement,
                        IFrameworkElementStatics,
                    > = windows_core::imp::FactoryCache::new();
                    SHARED.call(callback)
                }
            }
            impl windows_core::RuntimeType for FrameworkElement {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_class::<Self, IFrameworkElement>();
            }
            unsafe impl windows_core::Interface for FrameworkElement {
                type Vtable = <IFrameworkElement as windows_core::Interface>::Vtable;
                const IID: windows_core::GUID = <IFrameworkElement as windows_core::Interface>::IID;
            }
            impl windows_core::RuntimeName for FrameworkElement {
                const NAME: &'static str = "Microsoft.UI.Xaml.FrameworkElement";
            }
            unsafe impl Send for FrameworkElement {}
            unsafe impl Sync for FrameworkElement {}
            #[repr(C)]
            #[derive(Clone, Copy, Debug, Default, PartialEq)]
            pub struct GridLength {
                pub Value: f64,
                pub GridUnitType: GridUnitType,
            }
            impl windows_core::imp::TypeKind for GridLength {
                type TypeKind = windows_core::imp::CopyType;
            }
            impl windows_core::RuntimeType for GridLength {
                const SIGNATURE : windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice (b"struct(Microsoft.UI.Xaml.GridLength;f8;enum(Microsoft.UI.Xaml.GridUnitType;i4))") ;
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(b"Microsoft.UI.Xaml.GridLength");
            }
            #[repr(transparent)]
            #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
            pub struct GridUnitType(pub i32);
            impl GridUnitType {
                pub const Auto: Self = Self(0);
                pub const Pixel: Self = Self(1);
                pub const Star: Self = Self(2);
            }
            impl windows_core::imp::TypeKind for GridUnitType {
                type TypeKind = windows_core::imp::CopyType;
            }
            impl windows_core::RuntimeType for GridUnitType {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"enum(Microsoft.UI.Xaml.GridUnitType;i4)",
                    );
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(b"Microsoft.UI.Xaml.GridUnitType");
            }
            #[repr(transparent)]
            #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
            pub struct HorizontalAlignment(pub i32);
            impl HorizontalAlignment {
                pub const Left: Self = Self(0);
                pub const Center: Self = Self(1);
                pub const Right: Self = Self(2);
                pub const Stretch: Self = Self(3);
            }
            impl windows_core::imp::TypeKind for HorizontalAlignment {
                type TypeKind = windows_core::imp::CopyType;
            }
            impl windows_core::RuntimeType for HorizontalAlignment {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"enum(Microsoft.UI.Xaml.HorizontalAlignment;i4)",
                    );
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.HorizontalAlignment",
                    );
            }
            windows_core::imp::define_interface!(
                IApplication,
                IApplication_Vtbl,
                0x06a8f4e7_1146_55af_820d_ebd55643b021
            );
            impl windows_core::RuntimeType for IApplication {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(b"Microsoft.UI.Xaml.IApplication");
            }
            #[repr(C)]
            pub struct IApplication_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                pub Resources: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                ) -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IApplicationFactory,
                IApplicationFactory_Vtbl,
                0x9fd96657_5294_5a65_a1db_4fea143597da
            );
            impl windows_core::RuntimeType for IApplicationFactory {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IApplicationFactory",
                    );
            }
            #[repr(C)]
            pub struct IApplicationFactory_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                pub CreateInstance: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                )
                    -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IApplicationStatics,
                IApplicationStatics_Vtbl,
                0x4e0d09f5_4358_512c_a987_503b52848e95
            );
            impl windows_core::RuntimeType for IApplicationStatics {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IApplicationStatics",
                    );
            }
            #[repr(C)]
            pub struct IApplicationStatics_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
            }
            windows_core::imp::define_interface!(
                IDependencyObject,
                IDependencyObject_Vtbl,
                0xe7beaee7_160e_50f7_8789_d63463f979fa
            );
            impl windows_core::RuntimeType for IDependencyObject {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IDependencyObject",
                    );
            }
            #[repr(C)]
            pub struct IDependencyObject_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
            }
            windows_core::imp::define_interface!(
                IDependencyObjectFactory,
                IDependencyObjectFactory_Vtbl,
                0x936b614c_475f_5d7d_b3f7_bf1fbea28126
            );
            impl windows_core::RuntimeType for IDependencyObjectFactory {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IDependencyObjectFactory",
                    );
            }
            #[repr(C)]
            pub struct IDependencyObjectFactory_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
            }
            windows_core::imp::define_interface!(
                IFrameworkElement,
                IFrameworkElement_Vtbl,
                0xfe08f13d_dc6a_5495_ad44_c2d8d21863b0
            );
            impl windows_core::RuntimeType for IFrameworkElement {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IFrameworkElement",
                    );
            }
            #[repr(C)]
            pub struct IFrameworkElement_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                Triggers: usize,
                Resources: usize,
                SetResources: usize,
                Tag: usize,
                SetTag: usize,
                Language: usize,
                SetLanguage: usize,
                pub ActualWidth: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut f64,
                )
                    -> windows_core::HRESULT,
                pub ActualHeight: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut f64,
                )
                    -> windows_core::HRESULT,
                pub Width: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut f64,
                ) -> windows_core::HRESULT,
                pub SetWidth:
                    unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
                pub Height: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut f64,
                ) -> windows_core::HRESULT,
                pub SetHeight:
                    unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
                pub MinWidth: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut f64,
                ) -> windows_core::HRESULT,
                pub SetMinWidth:
                    unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
                MaxWidth: usize,
                SetMaxWidth: usize,
                pub MinHeight: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut f64,
                ) -> windows_core::HRESULT,
                pub SetMinHeight:
                    unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
                MaxHeight: usize,
                SetMaxHeight: usize,
                pub HorizontalAlignment: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut HorizontalAlignment,
                )
                    -> windows_core::HRESULT,
                pub SetHorizontalAlignment: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    HorizontalAlignment,
                )
                    -> windows_core::HRESULT,
                pub VerticalAlignment: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut VerticalAlignment,
                )
                    -> windows_core::HRESULT,
                pub SetVerticalAlignment: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    VerticalAlignment,
                )
                    -> windows_core::HRESULT,
                pub Margin: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut Thickness,
                ) -> windows_core::HRESULT,
                pub SetMargin: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    Thickness,
                ) -> windows_core::HRESULT,
                pub Name: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                ) -> windows_core::HRESULT,
                pub SetName: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                ) -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IFrameworkElementFactory,
                IFrameworkElementFactory_Vtbl,
                0xbd3f2272_3efa_5f92_b759_90b1cc3e784c
            );
            impl windows_core::RuntimeType for IFrameworkElementFactory {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IFrameworkElementFactory",
                    );
            }
            #[repr(C)]
            pub struct IFrameworkElementFactory_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
            }
            windows_core::imp::define_interface!(
                IFrameworkElementStatics,
                IFrameworkElementStatics_Vtbl,
                0x894e2704_14e7_569a_b21e_afc7df7145a1
            );
            impl windows_core::RuntimeType for IFrameworkElementStatics {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IFrameworkElementStatics",
                    );
            }
            #[repr(C)]
            pub struct IFrameworkElementStatics_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
            }
            windows_core::imp::define_interface!(
                IResourceDictionary,
                IResourceDictionary_Vtbl,
                0x1b690975_a710_5783_a6e1_15836f6186c2
            );
            impl windows_core::RuntimeType for IResourceDictionary {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IResourceDictionary",
                    );
            }
            #[repr(C)]
            pub struct IResourceDictionary_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                Source: usize,
                SetSource: usize,
                pub MergedDictionaries: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                )
                    -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IResourceDictionaryFactory,
                IResourceDictionaryFactory_Vtbl,
                0xea22a48f_ab71_56f6_a392_d82310c8aa7b
            );
            impl windows_core::RuntimeType for IResourceDictionaryFactory {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IResourceDictionaryFactory",
                    );
            }
            #[repr(C)]
            pub struct IResourceDictionaryFactory_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                pub CreateInstance: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                )
                    -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IRoutedEventArgs,
                IRoutedEventArgs_Vtbl,
                0x0908c407_1c7d_5de3_9c50_d971c62ec8ec
            );
            impl windows_core::RuntimeType for IRoutedEventArgs {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IRoutedEventArgs",
                    );
            }
            #[repr(C)]
            pub struct IRoutedEventArgs_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
            }
            windows_core::imp::define_interface!(
                IRoutedEventArgsFactory,
                IRoutedEventArgsFactory_Vtbl,
                0x914b02c7_076b_5b89_98e7_6c373379e9af
            );
            impl windows_core::RuntimeType for IRoutedEventArgsFactory {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IRoutedEventArgsFactory",
                    );
            }
            #[repr(C)]
            pub struct IRoutedEventArgsFactory_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
            }
            windows_core::imp::define_interface!(
                IUIElement,
                IUIElement_Vtbl,
                0xc3c01020_320c_5cf6_9d24_d396bbfa4d8b
            );
            impl windows_core::RuntimeType for IUIElement {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(b"Microsoft.UI.Xaml.IUIElement");
            }
            #[repr(C)]
            pub struct IUIElement_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                DesiredSize: usize,
                AllowDrop: usize,
                SetAllowDrop: usize,
                pub Opacity: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut f64,
                ) -> windows_core::HRESULT,
                pub SetOpacity:
                    unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
                Clip: usize,
                SetClip: usize,
                RenderTransform: usize,
                SetRenderTransform: usize,
                Projection: usize,
                SetProjection: usize,
                Transform3D: usize,
                SetTransform3D: usize,
                RenderTransformOrigin: usize,
                SetRenderTransformOrigin: usize,
                pub IsHitTestVisible: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut bool,
                )
                    -> windows_core::HRESULT,
                pub SetIsHitTestVisible: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    bool,
                )
                    -> windows_core::HRESULT,
                pub Visibility: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut Visibility,
                ) -> windows_core::HRESULT,
                pub SetVisibility: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    Visibility,
                )
                    -> windows_core::HRESULT,
                RenderSize: usize,
                UseLayoutRounding: usize,
                SetUseLayoutRounding: usize,
                Transitions: usize,
                SetTransitions: usize,
                CacheMode: usize,
                SetCacheMode: usize,
                IsTapEnabled: usize,
                SetIsTapEnabled: usize,
                IsDoubleTapEnabled: usize,
                SetIsDoubleTapEnabled: usize,
                CanDrag: usize,
                SetCanDrag: usize,
                IsRightTapEnabled: usize,
                SetIsRightTapEnabled: usize,
                IsHoldingEnabled: usize,
                SetIsHoldingEnabled: usize,
                ManipulationMode: usize,
                SetManipulationMode: usize,
                PointerCaptures: usize,
                ContextFlyout: usize,
                SetContextFlyout: usize,
                CompositeMode: usize,
                SetCompositeMode: usize,
                Lights: usize,
                CanBeScrollAnchor: usize,
                SetCanBeScrollAnchor: usize,
                ExitDisplayModeOnAccessKeyInvoked: usize,
                SetExitDisplayModeOnAccessKeyInvoked: usize,
                IsAccessKeyScope: usize,
                SetIsAccessKeyScope: usize,
                AccessKeyScopeOwner: usize,
                SetAccessKeyScopeOwner: usize,
                AccessKey: usize,
                SetAccessKey: usize,
                KeyTipPlacementMode: usize,
                SetKeyTipPlacementMode: usize,
                KeyTipHorizontalOffset: usize,
                SetKeyTipHorizontalOffset: usize,
                KeyTipVerticalOffset: usize,
                SetKeyTipVerticalOffset: usize,
                KeyTipTarget: usize,
                SetKeyTipTarget: usize,
                XYFocusKeyboardNavigation: usize,
                SetXYFocusKeyboardNavigation: usize,
                XYFocusUpNavigationStrategy: usize,
                SetXYFocusUpNavigationStrategy: usize,
                XYFocusDownNavigationStrategy: usize,
                SetXYFocusDownNavigationStrategy: usize,
                XYFocusLeftNavigationStrategy: usize,
                SetXYFocusLeftNavigationStrategy: usize,
                XYFocusRightNavigationStrategy: usize,
                SetXYFocusRightNavigationStrategy: usize,
                KeyboardAccelerators: usize,
                KeyboardAcceleratorPlacementTarget: usize,
                SetKeyboardAcceleratorPlacementTarget: usize,
                KeyboardAcceleratorPlacementMode: usize,
                SetKeyboardAcceleratorPlacementMode: usize,
                HighContrastAdjustment: usize,
                SetHighContrastAdjustment: usize,
                TabFocusNavigation: usize,
                SetTabFocusNavigation: usize,
                OpacityTransition: usize,
                SetOpacityTransition: usize,
                Translation: usize,
                SetTranslation: usize,
                TranslationTransition: usize,
                SetTranslationTransition: usize,
                Rotation: usize,
                SetRotation: usize,
                RotationTransition: usize,
                SetRotationTransition: usize,
                Scale: usize,
                SetScale: usize,
                ScaleTransition: usize,
                SetScaleTransition: usize,
                TransformMatrix: usize,
                SetTransformMatrix: usize,
                CenterPoint: usize,
                SetCenterPoint: usize,
                RotationAxis: usize,
                SetRotationAxis: usize,
                ActualOffset: usize,
                ActualSize: usize,
                XamlRoot: usize,
                SetXamlRoot: usize,
                Shadow: usize,
                SetShadow: usize,
                RasterizationScale: usize,
                SetRasterizationScale: usize,
                FocusState: usize,
                UseSystemFocusVisuals: usize,
                SetUseSystemFocusVisuals: usize,
                XYFocusLeft: usize,
                SetXYFocusLeft: usize,
                XYFocusRight: usize,
                SetXYFocusRight: usize,
                XYFocusUp: usize,
                SetXYFocusUp: usize,
                XYFocusDown: usize,
                SetXYFocusDown: usize,
                IsTabStop: usize,
                SetIsTabStop: usize,
                TabIndex: usize,
                SetTabIndex: usize,
                KeyUp: usize,
                RemoveKeyUp: usize,
                pub KeyDown: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                    *mut i64,
                ) -> windows_core::HRESULT,
                pub RemoveKeyDown:
                    unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IUIElementFactory,
                IUIElementFactory_Vtbl,
                0x14d1d309_add0_5ccb_b946_77488cd70f87
            );
            impl windows_core::RuntimeType for IUIElementFactory {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IUIElementFactory",
                    );
            }
            #[repr(C)]
            pub struct IUIElementFactory_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
            }
            windows_core::imp::define_interface!(
                IUIElementStatics,
                IUIElementStatics_Vtbl,
                0xd2921d87_3584_5e22_8a3a_c2c78dab4f6e
            );
            impl windows_core::RuntimeType for IUIElementStatics {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IUIElementStatics",
                    );
            }
            #[repr(C)]
            pub struct IUIElementStatics_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
            }
            windows_core::imp::define_interface!(
                IWindow,
                IWindow_Vtbl,
                0x61f0ec79_5d52_56b5_86fb_40fa4af288b0
            );
            impl windows_core::RuntimeType for IWindow {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(b"Microsoft.UI.Xaml.IWindow");
            }
            #[repr(C)]
            pub struct IWindow_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                Bounds: usize,
                Visible: usize,
                pub Content: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                ) -> windows_core::HRESULT,
                pub SetContent: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                ) -> windows_core::HRESULT,
                CoreWindow: usize,
                Compositor: usize,
                Dispatcher: usize,
                DispatcherQueue: usize,
                pub Title: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                ) -> windows_core::HRESULT,
                pub SetTitle: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                ) -> windows_core::HRESULT,
                ExtendsContentIntoTitleBar: usize,
                SetExtendsContentIntoTitleBar: usize,
                Activated: usize,
                RemoveActivated: usize,
                pub Closed: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                    *mut i64,
                ) -> windows_core::HRESULT,
                pub RemoveClosed:
                    unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
                SizeChanged: usize,
                RemoveSizeChanged: usize,
                VisibilityChanged: usize,
                RemoveVisibilityChanged: usize,
                pub Activate:
                    unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
                pub Close:
                    unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IWindowEventArgs,
                IWindowEventArgs_Vtbl,
                0x1140827c_fe0a_5268_bc2b_f4492c2ccb49
            );
            impl windows_core::RuntimeType for IWindowEventArgs {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.IWindowEventArgs",
                    );
            }
            #[repr(C)]
            pub struct IWindowEventArgs_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                pub Handled: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut bool,
                ) -> windows_core::HRESULT,
                pub SetHandled: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    bool,
                ) -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IWindowFactory,
                IWindowFactory_Vtbl,
                0xf0441536_afef_5222_918f_324a9b2dec75
            );
            impl windows_core::RuntimeType for IWindowFactory {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(b"Microsoft.UI.Xaml.IWindowFactory");
            }
            #[repr(C)]
            pub struct IWindowFactory_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
                pub CreateInstance: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                    *mut *mut core::ffi::c_void,
                )
                    -> windows_core::HRESULT,
            }
            windows_core::imp::define_interface!(
                IWindowStatics,
                IWindowStatics_Vtbl,
                0x8cc985e3_a41a_5df4_b531_d3a1788d86c5
            );
            impl windows_core::RuntimeType for IWindowStatics {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(b"Microsoft.UI.Xaml.IWindowStatics");
            }
            #[repr(C)]
            pub struct IWindowStatics_Vtbl {
                pub base__: windows_core::IInspectable_Vtbl,
            }
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct ResourceDictionary(windows_core::IUnknown);
            windows_core::imp::interface_hierarchy!(
                ResourceDictionary,
                windows_core::IUnknown,
                windows_core::IInspectable
            );
            windows_core::imp::required_hierarchy!(
                ResourceDictionary,
                super::super::super::Windows::Foundation::Collections::IIterable<
                    windows_collections::IKeyValuePair<
                        windows_core::IInspectable,
                        windows_core::IInspectable,
                    >,
                >,
                DependencyObject
            );
            impl ResourceDictionary {
                pub fn MergedDictionaries(
                    &self,
                ) -> windows_core::Result<
                    super::super::super::Windows::Foundation::Collections::IVector<Self>,
                > {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).MergedDictionaries)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .and_then(|| windows_core::imp::Type::from_abi(result__))
                    }
                }
                pub fn new() -> windows_core::Result<Self> {
                    Self::IResourceDictionaryFactory(|this| unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(this).CreateInstance)(
                            windows_core::Interface::as_raw(this),
                            core::ptr::null_mut(),
                            core::ptr::null_mut(),
                            &mut result__,
                        )
                        .and_then(|| windows_core::imp::Type::from_abi(result__))
                    })
                }
                pub fn compose<T>(compose: T) -> windows_core::Result<Self>
                where
                    T: windows_core::Compose,
                {
                    Self::IResourceDictionaryFactory(|this| unsafe {
                        let (derived__, base__) = windows_core::Compose::compose(compose);
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(this).CreateInstance)(
                            windows_core::Interface::as_raw(this),
                            core::mem::transmute_copy(&derived__),
                            base__ as *mut _ as _,
                            &mut result__,
                        )
                        .ok()?;
                        let _ = &derived__;
                        windows_core::imp::Type::from_abi(result__)
                    })
                }
                fn IResourceDictionaryFactory<
                    R,
                    F: FnOnce(&IResourceDictionaryFactory) -> windows_core::Result<R>,
                >(
                    callback: F,
                ) -> windows_core::Result<R> {
                    static SHARED: windows_core::imp::FactoryCache<
                        ResourceDictionary,
                        IResourceDictionaryFactory,
                    > = windows_core::imp::FactoryCache::new();
                    SHARED.call(callback)
                }
            }
            impl windows_core::RuntimeType for ResourceDictionary {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_class::<Self, IResourceDictionary>();
            }
            unsafe impl windows_core::Interface for ResourceDictionary {
                type Vtable = <IResourceDictionary as windows_core::Interface>::Vtable;
                const IID: windows_core::GUID =
                    <IResourceDictionary as windows_core::Interface>::IID;
            }
            impl windows_core::RuntimeName for ResourceDictionary {
                const NAME: &'static str = "Microsoft.UI.Xaml.ResourceDictionary";
            }
            unsafe impl Send for ResourceDictionary {}
            unsafe impl Sync for ResourceDictionary {}
            impl IntoIterator for ResourceDictionary {
                type Item = windows_collections::IKeyValuePair<
                    windows_core::IInspectable,
                    windows_core::IInspectable,
                >;
                type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                fn into_iter(self) -> Self::IntoIter {
                    IntoIterator::into_iter(&self)
                }
            }
            impl IntoIterator for &ResourceDictionary {
                type Item = windows_collections::IKeyValuePair<
                    windows_core::IInspectable,
                    windows_core::IInspectable,
                >;
                type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                fn into_iter(self) -> Self::IntoIter {
                    windows_collections::BufferedIterator::new(self.First().unwrap())
                }
            }
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct RoutedEventArgs(windows_core::IUnknown);
            windows_core::imp::interface_hierarchy!(
                RoutedEventArgs,
                windows_core::IUnknown,
                windows_core::IInspectable
            );
            impl RoutedEventArgs {
                fn IRoutedEventArgsFactory<
                    R,
                    F: FnOnce(&IRoutedEventArgsFactory) -> windows_core::Result<R>,
                >(
                    callback: F,
                ) -> windows_core::Result<R> {
                    static SHARED: windows_core::imp::FactoryCache<
                        RoutedEventArgs,
                        IRoutedEventArgsFactory,
                    > = windows_core::imp::FactoryCache::new();
                    SHARED.call(callback)
                }
            }
            impl windows_core::RuntimeType for RoutedEventArgs {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_class::<Self, IRoutedEventArgs>();
            }
            unsafe impl windows_core::Interface for RoutedEventArgs {
                type Vtable = <IRoutedEventArgs as windows_core::Interface>::Vtable;
                const IID: windows_core::GUID = <IRoutedEventArgs as windows_core::Interface>::IID;
            }
            impl windows_core::RuntimeName for RoutedEventArgs {
                const NAME: &'static str = "Microsoft.UI.Xaml.RoutedEventArgs";
            }
            unsafe impl Send for RoutedEventArgs {}
            unsafe impl Sync for RoutedEventArgs {}
            windows_core::imp::define_interface!(
                RoutedEventHandler,
                RoutedEventHandler_Vtbl,
                0xdae23d85_69ca_5bdf_805b_6161a3a215cc
            );
            impl windows_core::RuntimeType for RoutedEventHandler {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_interface::<Self>();
            }
            impl RoutedEventHandler {
                pub fn new<
                    F: Fn(
                            windows_core::Ref<windows_core::IInspectable>,
                            windows_core::Ref<RoutedEventArgs>,
                        ) -> windows_core::Result<()>
                        + Send
                        + 'static,
                >(
                    invoke: F,
                ) -> Self {
                    let com = windows_core::imp::DelegateBox::<Self, F>::new(
                        &RoutedEventHandlerBox::<F>::VTABLE,
                        invoke,
                    );
                    unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
                }
                pub fn Invoke<P0, P1>(&self, sender: P0, e: P1) -> windows_core::Result<()>
                where
                    P0: windows_core::Param<windows_core::IInspectable>,
                    P1: windows_core::Param<RoutedEventArgs>,
                {
                    unsafe {
                        (windows_core::Interface::vtable(self).Invoke)(
                            windows_core::Interface::as_raw(self),
                            sender.param().abi(),
                            e.param().abi(),
                        )
                        .ok()
                    }
                }
            }
            #[repr(C)]
            pub struct RoutedEventHandler_Vtbl {
                base__: windows_core::IUnknown_Vtbl,
                Invoke: unsafe extern "system" fn(
                    this: *mut core::ffi::c_void,
                    sender: *mut core::ffi::c_void,
                    e: *mut core::ffi::c_void,
                ) -> windows_core::HRESULT,
            }
            struct RoutedEventHandlerBox<
                F: Fn(
                        windows_core::Ref<windows_core::IInspectable>,
                        windows_core::Ref<RoutedEventArgs>,
                    ) -> windows_core::Result<()>
                    + Send
                    + 'static,
            >(core::marker::PhantomData<(fn() -> F,)>);
            impl<
                F: Fn(
                        windows_core::Ref<windows_core::IInspectable>,
                        windows_core::Ref<RoutedEventArgs>,
                    ) -> windows_core::Result<()>
                    + Send
                    + 'static,
            > RoutedEventHandlerBox<F>
            {
                const VTABLE: RoutedEventHandler_Vtbl = RoutedEventHandler_Vtbl {
                    base__: windows_core::IUnknown_Vtbl {
                        QueryInterface:
                            windows_core::imp::DelegateBox::<RoutedEventHandler, F>::QueryInterface,
                        AddRef: windows_core::imp::DelegateBox::<RoutedEventHandler, F>::AddRef,
                        Release: windows_core::imp::DelegateBox::<RoutedEventHandler, F>::Release,
                    },
                    Invoke: Self::Invoke,
                };
                unsafe extern "system" fn Invoke(
                    this: *mut core::ffi::c_void,
                    sender: *mut core::ffi::c_void,
                    e: *mut core::ffi::c_void,
                ) -> windows_core::HRESULT {
                    unsafe {
                        let this = &mut *(this as *mut *mut core::ffi::c_void
                            as *mut windows_core::imp::DelegateBox<RoutedEventHandler, F>);
                        (this.invoke)(
                            core::mem::transmute_copy(&sender),
                            core::mem::transmute_copy(&e),
                        )
                        .into()
                    }
                }
            }
            #[repr(transparent)]
            #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
            pub struct TextWrapping(pub i32);
            impl TextWrapping {
                pub const NoWrap: Self = Self(1);
                pub const Wrap: Self = Self(2);
                pub const WrapWholeWords: Self = Self(3);
            }
            impl windows_core::imp::TypeKind for TextWrapping {
                type TypeKind = windows_core::imp::CopyType;
            }
            impl windows_core::RuntimeType for TextWrapping {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"enum(Microsoft.UI.Xaml.TextWrapping;i4)",
                    );
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(b"Microsoft.UI.Xaml.TextWrapping");
            }
            #[repr(C)]
            #[derive(Clone, Copy, Debug, Default, PartialEq)]
            pub struct Thickness {
                pub Left: f64,
                pub Top: f64,
                pub Right: f64,
                pub Bottom: f64,
            }
            impl windows_core::imp::TypeKind for Thickness {
                type TypeKind = windows_core::imp::CopyType;
            }
            impl windows_core::RuntimeType for Thickness {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"struct(Microsoft.UI.Xaml.Thickness;f8;f8;f8;f8)",
                    );
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(b"Microsoft.UI.Xaml.Thickness");
            }
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct UIElement(windows_core::IUnknown);
            windows_core::imp::interface_hierarchy!(
                UIElement,
                windows_core::IUnknown,
                windows_core::IInspectable
            );
            windows_core::imp::required_hierarchy!(UIElement, DependencyObject);
            impl UIElement {
                pub fn Opacity(&self) -> windows_core::Result<f64> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).Opacity)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetOpacity)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).IsHitTestVisible)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetIsHitTestVisible)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn Visibility(&self) -> windows_core::Result<Visibility> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).Visibility)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetVisibility(&self, value: Visibility) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetVisibility)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
                pub fn KeyDown<F>(
                    &self,
                    handler: F,
                ) -> windows_core::Result<windows_core::EventRevoker>
                where
                    F: Fn(
                            windows_core::Ref<windows_core::IInspectable>,
                            windows_core::Ref<Input::KeyRoutedEventArgs>,
                        ) + Send
                        + 'static,
                {
                    let handler = <Input::KeyEventHandler>::new(move |a0, a1| {
                        handler(a0, a1);
                        Ok(())
                    });
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        let token__ = (windows_core::Interface::vtable(self).KeyDown)(
                            windows_core::Interface::as_raw(self),
                            windows_core::Interface::as_raw(&handler),
                            &mut result__,
                        )
                        .map(|| result__)?;
                        Ok(windows_core::EventRevoker::new(
                            self.clone(),
                            token__,
                            windows_core::Interface::vtable(self).RemoveKeyDown,
                        ))
                    }
                }
                fn IUIElementStatics<
                    R,
                    F: FnOnce(&IUIElementStatics) -> windows_core::Result<R>,
                >(
                    callback: F,
                ) -> windows_core::Result<R> {
                    static SHARED: windows_core::imp::FactoryCache<UIElement, IUIElementStatics> =
                        windows_core::imp::FactoryCache::new();
                    SHARED.call(callback)
                }
            }
            impl windows_core::RuntimeType for UIElement {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_class::<Self, IUIElement>();
            }
            unsafe impl windows_core::Interface for UIElement {
                type Vtable = <IUIElement as windows_core::Interface>::Vtable;
                const IID: windows_core::GUID = <IUIElement as windows_core::Interface>::IID;
            }
            impl windows_core::RuntimeName for UIElement {
                const NAME: &'static str = "Microsoft.UI.Xaml.UIElement";
            }
            unsafe impl Send for UIElement {}
            unsafe impl Sync for UIElement {}
            #[repr(transparent)]
            #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
            pub struct VerticalAlignment(pub i32);
            impl VerticalAlignment {
                pub const Top: Self = Self(0);
                pub const Center: Self = Self(1);
                pub const Bottom: Self = Self(2);
                pub const Stretch: Self = Self(3);
            }
            impl windows_core::imp::TypeKind for VerticalAlignment {
                type TypeKind = windows_core::imp::CopyType;
            }
            impl windows_core::RuntimeType for VerticalAlignment {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"enum(Microsoft.UI.Xaml.VerticalAlignment;i4)",
                    );
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"Microsoft.UI.Xaml.VerticalAlignment",
                    );
            }
            #[repr(transparent)]
            #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
            pub struct Visibility(pub i32);
            impl Visibility {
                pub const Visible: Self = Self(0);
                pub const Collapsed: Self = Self(1);
            }
            impl windows_core::imp::TypeKind for Visibility {
                type TypeKind = windows_core::imp::CopyType;
            }
            impl windows_core::RuntimeType for Visibility {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(
                        b"enum(Microsoft.UI.Xaml.Visibility;i4)",
                    );
                const NAME: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::from_slice(b"Microsoft.UI.Xaml.Visibility");
            }
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct Window(windows_core::IUnknown);
            windows_core::imp::interface_hierarchy!(
                Window,
                windows_core::IUnknown,
                windows_core::IInspectable
            );
            impl Window {
                pub fn Content(&self) -> windows_core::Result<UIElement> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).Content)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .and_then(|| windows_core::imp::Type::from_abi(result__))
                    }
                }
                pub fn SetContent<P0>(&self, value: P0) -> windows_core::Result<()>
                where
                    P0: windows_core::Param<UIElement>,
                {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetContent)(
                            windows_core::Interface::as_raw(self),
                            value.param().abi(),
                        )
                        .ok()
                    }
                }
                pub fn Title(&self) -> windows_core::Result<windows_core::HSTRING> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).Title)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| core::mem::transmute(result__))
                    }
                }
                pub fn SetTitle(&self, value: &windows_core::HSTRING) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetTitle)(
                            windows_core::Interface::as_raw(self),
                            core::mem::transmute_copy(value),
                        )
                        .ok()
                    }
                }
                pub fn Closed<F>(
                    &self,
                    handler: F,
                ) -> windows_core::Result<windows_core::EventRevoker>
                where
                    F: Fn(
                            windows_core::Ref<windows_core::IInspectable>,
                            windows_core::Ref<WindowEventArgs>,
                        ) + Send
                        + 'static,
                {
                    let handler = <super::super::super::Windows::Foundation::TypedEventHandler<
                        windows_core::IInspectable,
                        WindowEventArgs,
                    >>::new(move |a0, a1| {
                        handler(a0, a1);
                        Ok(())
                    });
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        let token__ = (windows_core::Interface::vtable(self).Closed)(
                            windows_core::Interface::as_raw(self),
                            windows_core::Interface::as_raw(&handler),
                            &mut result__,
                        )
                        .map(|| result__)?;
                        Ok(windows_core::EventRevoker::new(
                            self.clone(),
                            token__,
                            windows_core::Interface::vtable(self).RemoveClosed,
                        ))
                    }
                }
                pub fn Activate(&self) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).Activate)(
                            windows_core::Interface::as_raw(self),
                        )
                        .ok()
                    }
                }
                pub fn Close(&self) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).Close)(
                            windows_core::Interface::as_raw(self),
                        )
                        .ok()
                    }
                }
                pub fn new() -> windows_core::Result<Self> {
                    Self::IWindowFactory(|this| unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(this).CreateInstance)(
                            windows_core::Interface::as_raw(this),
                            core::ptr::null_mut(),
                            core::ptr::null_mut(),
                            &mut result__,
                        )
                        .and_then(|| windows_core::imp::Type::from_abi(result__))
                    })
                }
                pub fn compose<T>(compose: T) -> windows_core::Result<Self>
                where
                    T: windows_core::Compose,
                {
                    Self::IWindowFactory(|this| unsafe {
                        let (derived__, base__) = windows_core::Compose::compose(compose);
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(this).CreateInstance)(
                            windows_core::Interface::as_raw(this),
                            core::mem::transmute_copy(&derived__),
                            base__ as *mut _ as _,
                            &mut result__,
                        )
                        .ok()?;
                        let _ = &derived__;
                        windows_core::imp::Type::from_abi(result__)
                    })
                }
                fn IWindowFactory<R, F: FnOnce(&IWindowFactory) -> windows_core::Result<R>>(
                    callback: F,
                ) -> windows_core::Result<R> {
                    static SHARED: windows_core::imp::FactoryCache<Window, IWindowFactory> =
                        windows_core::imp::FactoryCache::new();
                    SHARED.call(callback)
                }
                fn IWindowStatics<R, F: FnOnce(&IWindowStatics) -> windows_core::Result<R>>(
                    callback: F,
                ) -> windows_core::Result<R> {
                    static SHARED: windows_core::imp::FactoryCache<Window, IWindowStatics> =
                        windows_core::imp::FactoryCache::new();
                    SHARED.call(callback)
                }
            }
            impl windows_core::RuntimeType for Window {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_class::<Self, IWindow>();
            }
            unsafe impl windows_core::Interface for Window {
                type Vtable = <IWindow as windows_core::Interface>::Vtable;
                const IID: windows_core::GUID = <IWindow as windows_core::Interface>::IID;
            }
            impl windows_core::RuntimeName for Window {
                const NAME: &'static str = "Microsoft.UI.Xaml.Window";
            }
            unsafe impl Send for Window {}
            unsafe impl Sync for Window {}
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct WindowEventArgs(windows_core::IUnknown);
            windows_core::imp::interface_hierarchy!(
                WindowEventArgs,
                windows_core::IUnknown,
                windows_core::IInspectable
            );
            impl WindowEventArgs {
                pub fn Handled(&self) -> windows_core::Result<bool> {
                    unsafe {
                        let mut result__ = core::mem::zeroed();
                        (windows_core::Interface::vtable(self).Handled)(
                            windows_core::Interface::as_raw(self),
                            &mut result__,
                        )
                        .map(|| result__)
                    }
                }
                pub fn SetHandled(&self, value: bool) -> windows_core::Result<()> {
                    unsafe {
                        (windows_core::Interface::vtable(self).SetHandled)(
                            windows_core::Interface::as_raw(self),
                            value,
                        )
                        .ok()
                    }
                }
            }
            impl windows_core::RuntimeType for WindowEventArgs {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::for_class::<Self, IWindowEventArgs>();
            }
            unsafe impl windows_core::Interface for WindowEventArgs {
                type Vtable = <IWindowEventArgs as windows_core::Interface>::Vtable;
                const IID: windows_core::GUID = <IWindowEventArgs as windows_core::Interface>::IID;
            }
            impl windows_core::RuntimeName for WindowEventArgs {
                const NAME: &'static str = "Microsoft.UI.Xaml.WindowEventArgs";
            }
            unsafe impl Send for WindowEventArgs {}
            unsafe impl Sync for WindowEventArgs {}
            pub mod Automation {
                pub mod Peers {
                    #[repr(transparent)]
                    #[derive(Clone, Debug, Eq, PartialEq)]
                    pub struct AutomationPeer(windows_core::IUnknown);
                    windows_core::imp::interface_hierarchy!(
                        AutomationPeer,
                        windows_core::IUnknown,
                        windows_core::IInspectable
                    );
                    windows_core::imp::required_hierarchy!(
                        AutomationPeer,
                        super::super::DependencyObject
                    );
                    impl AutomationPeer {
                        fn IAutomationPeerFactory<
                            R,
                            F: FnOnce(&IAutomationPeerFactory) -> windows_core::Result<R>,
                        >(
                            callback: F,
                        ) -> windows_core::Result<R> {
                            static SHARED: windows_core::imp::FactoryCache<
                                AutomationPeer,
                                IAutomationPeerFactory,
                            > = windows_core::imp::FactoryCache::new();
                            SHARED.call(callback)
                        }
                        fn IAutomationPeerStatics<
                            R,
                            F: FnOnce(&IAutomationPeerStatics) -> windows_core::Result<R>,
                        >(
                            callback: F,
                        ) -> windows_core::Result<R> {
                            static SHARED: windows_core::imp::FactoryCache<
                                AutomationPeer,
                                IAutomationPeerStatics,
                            > = windows_core::imp::FactoryCache::new();
                            SHARED.call(callback)
                        }
                    }
                    impl windows_core::RuntimeType for AutomationPeer {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_class::<Self, IAutomationPeer>();
                    }
                    unsafe impl windows_core::Interface for AutomationPeer {
                        type Vtable = <IAutomationPeer as windows_core::Interface>::Vtable;
                        const IID: windows_core::GUID =
                            <IAutomationPeer as windows_core::Interface>::IID;
                    }
                    impl windows_core::RuntimeName for AutomationPeer {
                        const NAME: &'static str =
                            "Microsoft.UI.Xaml.Automation.Peers.AutomationPeer";
                    }
                    unsafe impl Send for AutomationPeer {}
                    unsafe impl Sync for AutomationPeer {}
                    #[repr(transparent)]
                    #[derive(Clone, Debug, Eq, PartialEq)]
                    pub struct ButtonAutomationPeer(windows_core::IUnknown);
                    windows_core::imp::interface_hierarchy!(
                        ButtonAutomationPeer,
                        windows_core::IUnknown,
                        windows_core::IInspectable
                    );
                    windows_core::imp::required_hierarchy!(
                        ButtonAutomationPeer,
                        super::Provider::IInvokeProvider,
                        ButtonBaseAutomationPeer,
                        FrameworkElementAutomationPeer,
                        AutomationPeer,
                        super::super::DependencyObject
                    );
                    impl ButtonAutomationPeer {
                        pub fn CreateInstanceWithOwner<P0>(owner: P0) -> windows_core::Result<Self>
                        where
                            P0: windows_core::Param<super::super::Controls::Button>,
                        {
                            Self::IButtonAutomationPeerFactory(|this| unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).CreateInstanceWithOwner)(
                                    windows_core::Interface::as_raw(this),
                                    owner.param().abi(),
                                    core::ptr::null_mut(),
                                    core::ptr::null_mut(),
                                    &mut result__,
                                )
                                .and_then(|| windows_core::imp::Type::from_abi(result__))
                            })
                        }
                        pub fn CreateInstanceWithOwner_compose<P0, T>(
                            owner: P0,
                            compose: T,
                        ) -> windows_core::Result<Self>
                        where
                            P0: windows_core::Param<super::super::Controls::Button>,
                            T: windows_core::Compose,
                        {
                            Self::IButtonAutomationPeerFactory(|this| unsafe {
                                let (derived__, base__) = windows_core::Compose::compose(compose);
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).CreateInstanceWithOwner)(
                                    windows_core::Interface::as_raw(this),
                                    owner.param().abi(),
                                    core::mem::transmute_copy(&derived__),
                                    base__ as *mut _ as _,
                                    &mut result__,
                                )
                                .ok()?;
                                let _ = &derived__;
                                windows_core::imp::Type::from_abi(result__)
                            })
                        }
                        pub fn Invoke(&self) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::Provider::IInvokeProvider,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).Invoke)(
                                    windows_core::Interface::as_raw(this),
                                )
                                .ok()
                            }
                        }
                        fn IButtonAutomationPeerFactory<
                            R,
                            F: FnOnce(&IButtonAutomationPeerFactory) -> windows_core::Result<R>,
                        >(
                            callback: F,
                        ) -> windows_core::Result<R> {
                            static SHARED: windows_core::imp::FactoryCache<
                                ButtonAutomationPeer,
                                IButtonAutomationPeerFactory,
                            > = windows_core::imp::FactoryCache::new();
                            SHARED.call(callback)
                        }
                    }
                    impl windows_core::RuntimeType for ButtonAutomationPeer {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_class::<Self, IButtonAutomationPeer>(
                            );
                    }
                    unsafe impl windows_core::Interface for ButtonAutomationPeer {
                        type Vtable = <IButtonAutomationPeer as windows_core::Interface>::Vtable;
                        const IID: windows_core::GUID =
                            <IButtonAutomationPeer as windows_core::Interface>::IID;
                    }
                    impl windows_core::RuntimeName for ButtonAutomationPeer {
                        const NAME: &'static str =
                            "Microsoft.UI.Xaml.Automation.Peers.ButtonAutomationPeer";
                    }
                    unsafe impl Send for ButtonAutomationPeer {}
                    unsafe impl Sync for ButtonAutomationPeer {}
                    #[repr(transparent)]
                    #[derive(Clone, Debug, Eq, PartialEq)]
                    pub struct ButtonBaseAutomationPeer(windows_core::IUnknown);
                    windows_core::imp::interface_hierarchy!(
                        ButtonBaseAutomationPeer,
                        windows_core::IUnknown,
                        windows_core::IInspectable
                    );
                    windows_core::imp::required_hierarchy!(
                        ButtonBaseAutomationPeer,
                        FrameworkElementAutomationPeer,
                        AutomationPeer,
                        super::super::DependencyObject
                    );
                    impl ButtonBaseAutomationPeer {
                        fn IButtonBaseAutomationPeerFactory<
                            R,
                            F: FnOnce(&IButtonBaseAutomationPeerFactory) -> windows_core::Result<R>,
                        >(
                            callback: F,
                        ) -> windows_core::Result<R> {
                            static SHARED: windows_core::imp::FactoryCache<
                                ButtonBaseAutomationPeer,
                                IButtonBaseAutomationPeerFactory,
                            > = windows_core::imp::FactoryCache::new();
                            SHARED.call(callback)
                        }
                    }
                    impl windows_core::RuntimeType for ButtonBaseAutomationPeer {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_class::<
                                Self,
                                IButtonBaseAutomationPeer,
                            >();
                    }
                    unsafe impl windows_core::Interface for ButtonBaseAutomationPeer {
                        type Vtable =
                            <IButtonBaseAutomationPeer as windows_core::Interface>::Vtable;
                        const IID: windows_core::GUID =
                            <IButtonBaseAutomationPeer as windows_core::Interface>::IID;
                    }
                    impl windows_core::RuntimeName for ButtonBaseAutomationPeer {
                        const NAME: &'static str =
                            "Microsoft.UI.Xaml.Automation.Peers.ButtonBaseAutomationPeer";
                    }
                    unsafe impl Send for ButtonBaseAutomationPeer {}
                    unsafe impl Sync for ButtonBaseAutomationPeer {}
                    #[repr(transparent)]
                    #[derive(Clone, Debug, Eq, PartialEq)]
                    pub struct FrameworkElementAutomationPeer(windows_core::IUnknown);
                    windows_core::imp::interface_hierarchy!(
                        FrameworkElementAutomationPeer,
                        windows_core::IUnknown,
                        windows_core::IInspectable
                    );
                    windows_core::imp::required_hierarchy!(
                        FrameworkElementAutomationPeer,
                        AutomationPeer,
                        super::super::DependencyObject
                    );
                    impl FrameworkElementAutomationPeer {
                        fn IFrameworkElementAutomationPeerFactory<
                            R,
                            F: FnOnce(
                                &IFrameworkElementAutomationPeerFactory,
                            ) -> windows_core::Result<R>,
                        >(
                            callback: F,
                        ) -> windows_core::Result<R> {
                            static SHARED: windows_core::imp::FactoryCache<
                                FrameworkElementAutomationPeer,
                                IFrameworkElementAutomationPeerFactory,
                            > = windows_core::imp::FactoryCache::new();
                            SHARED.call(callback)
                        }
                        fn IFrameworkElementAutomationPeerStatics<
                            R,
                            F: FnOnce(
                                &IFrameworkElementAutomationPeerStatics,
                            ) -> windows_core::Result<R>,
                        >(
                            callback: F,
                        ) -> windows_core::Result<R> {
                            static SHARED: windows_core::imp::FactoryCache<
                                FrameworkElementAutomationPeer,
                                IFrameworkElementAutomationPeerStatics,
                            > = windows_core::imp::FactoryCache::new();
                            SHARED.call(callback)
                        }
                    }
                    impl windows_core::RuntimeType for FrameworkElementAutomationPeer {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_class::<
                                Self,
                                IFrameworkElementAutomationPeer,
                            >();
                    }
                    unsafe impl windows_core::Interface for FrameworkElementAutomationPeer {
                        type Vtable =
                            <IFrameworkElementAutomationPeer as windows_core::Interface>::Vtable;
                        const IID: windows_core::GUID =
                            <IFrameworkElementAutomationPeer as windows_core::Interface>::IID;
                    }
                    impl windows_core::RuntimeName for FrameworkElementAutomationPeer {
                        const NAME: &'static str =
                            "Microsoft.UI.Xaml.Automation.Peers.FrameworkElementAutomationPeer";
                    }
                    unsafe impl Send for FrameworkElementAutomationPeer {}
                    unsafe impl Sync for FrameworkElementAutomationPeer {}
                    windows_core::imp::define_interface!(
                        IAutomationPeer,
                        IAutomationPeer_Vtbl,
                        0xe51d3e4e_34f0_568c_999f_6277e2afe6d7
                    );
                    impl windows_core::RuntimeType for IAutomationPeer {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Automation.Peers.IAutomationPeer",
                            );
                    }
                    #[repr(C)]
                    pub struct IAutomationPeer_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IAutomationPeerFactory,
                        IAutomationPeerFactory_Vtbl,
                        0xa1af86a0_6ec6_5be2_858f_72808be6fddd
                    );
                    impl windows_core::RuntimeType for IAutomationPeerFactory {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Automation.Peers.IAutomationPeerFactory",
                            );
                    }
                    #[repr(C)]
                    pub struct IAutomationPeerFactory_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IAutomationPeerStatics,
                        IAutomationPeerStatics_Vtbl,
                        0x90b157ff_18d5_5623_850c_612ceae576bd
                    );
                    impl windows_core::RuntimeType for IAutomationPeerStatics {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Automation.Peers.IAutomationPeerStatics",
                            );
                    }
                    #[repr(C)]
                    pub struct IAutomationPeerStatics_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IButtonAutomationPeer,
                        IButtonAutomationPeer_Vtbl,
                        0xa3a2e96b_8a2d_50bf_9b3d_dfbc4653f5a6
                    );
                    impl windows_core::RuntimeType for IButtonAutomationPeer {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Automation.Peers.IButtonAutomationPeer",
                            );
                    }
                    #[repr(C)]
                    pub struct IButtonAutomationPeer_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IButtonAutomationPeerFactory,
                        IButtonAutomationPeerFactory_Vtbl,
                        0x1c2f87b7_fbc1_56b0_a654_550d61c423d1
                    );
                    impl windows_core::RuntimeType for IButtonAutomationPeerFactory {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Automation.Peers.IButtonAutomationPeerFactory",
                            );
                    }
                    #[repr(C)]
                    pub struct IButtonAutomationPeerFactory_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                        pub CreateInstanceWithOwner:
                            unsafe extern "system" fn(
                                *mut core::ffi::c_void,
                                *mut core::ffi::c_void,
                                *mut core::ffi::c_void,
                                *mut *mut core::ffi::c_void,
                                *mut *mut core::ffi::c_void,
                            )
                                -> windows_core::HRESULT,
                    }
                    windows_core::imp::define_interface!(
                        IButtonBaseAutomationPeer,
                        IButtonBaseAutomationPeer_Vtbl,
                        0x7a88a2d8_f8dd_5526_8a40_e2e3888193bf
                    );
                    impl windows_core::RuntimeType for IButtonBaseAutomationPeer {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Automation.Peers.IButtonBaseAutomationPeer",
                            );
                    }
                    #[repr(C)]
                    pub struct IButtonBaseAutomationPeer_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IButtonBaseAutomationPeerFactory,
                        IButtonBaseAutomationPeerFactory_Vtbl,
                        0xd6e707bb_7299_5f7a_9c03_7fa10f939771
                    );
                    impl windows_core::RuntimeType for IButtonBaseAutomationPeerFactory {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME : windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice (b"Microsoft.UI.Xaml.Automation.Peers.IButtonBaseAutomationPeerFactory") ;
                    }
                    #[repr(C)]
                    pub struct IButtonBaseAutomationPeerFactory_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IFrameworkElementAutomationPeer,
                        IFrameworkElementAutomationPeer_Vtbl,
                        0x7dab4f24_605c_51cb_87db_3eed1b9fb37b
                    );
                    impl windows_core::RuntimeType for IFrameworkElementAutomationPeer {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME : windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice (b"Microsoft.UI.Xaml.Automation.Peers.IFrameworkElementAutomationPeer") ;
                    }
                    #[repr(C)]
                    pub struct IFrameworkElementAutomationPeer_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IFrameworkElementAutomationPeerFactory,
                        IFrameworkElementAutomationPeerFactory_Vtbl,
                        0x1682c3f8_238d_5c7e_a5a5_08cc3c10ac7c
                    );
                    impl windows_core::RuntimeType for IFrameworkElementAutomationPeerFactory {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME : windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice (b"Microsoft.UI.Xaml.Automation.Peers.IFrameworkElementAutomationPeerFactory") ;
                    }
                    #[repr(C)]
                    pub struct IFrameworkElementAutomationPeerFactory_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IFrameworkElementAutomationPeerStatics,
                        IFrameworkElementAutomationPeerStatics_Vtbl,
                        0x081f6fbe_6500_528a_a506_f5a4d41ddf6c
                    );
                    impl windows_core::RuntimeType for IFrameworkElementAutomationPeerStatics {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME : windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice (b"Microsoft.UI.Xaml.Automation.Peers.IFrameworkElementAutomationPeerStatics") ;
                    }
                    #[repr(C)]
                    pub struct IFrameworkElementAutomationPeerStatics_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                }
                pub mod Provider {
                    windows_core::imp::define_interface!(
                        IInvokeProvider,
                        IInvokeProvider_Vtbl,
                        0x02481105_3378_544d_b4e1_a1b368afbc02
                    );
                    impl windows_core::RuntimeType for IInvokeProvider {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Automation.Provider.IInvokeProvider",
                            );
                    }
                    windows_core::imp::interface_hierarchy!(
                        IInvokeProvider,
                        windows_core::IUnknown,
                        windows_core::IInspectable
                    );
                    impl IInvokeProvider {
                        pub fn Invoke(&self) -> windows_core::Result<()> {
                            unsafe {
                                (windows_core::Interface::vtable(self).Invoke)(
                                    windows_core::Interface::as_raw(self),
                                )
                                .ok()
                            }
                        }
                    }
                    #[repr(C)]
                    pub struct IInvokeProvider_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                        pub Invoke: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                        )
                            -> windows_core::HRESULT,
                    }
                }
            }
            pub mod Controls {
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct Button(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    Button,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    Button,
                    Primitives::ButtonBase,
                    ContentControl,
                    Control,
                    super::FrameworkElement,
                    super::UIElement,
                    super::DependencyObject
                );
                impl Button {
                    pub fn Click<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::RoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<Primitives::IButtonBase>(self)?;
                        let handler = <super::RoutedEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).Click)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveClick,
                            ))
                        }
                    }
                    pub fn new() -> windows_core::Result<Self> {
                        Self::IButtonFactory(|this| unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).CreateInstance)(
                                windows_core::Interface::as_raw(this),
                                core::ptr::null_mut(),
                                core::ptr::null_mut(),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        })
                    }
                    pub fn compose<T>(compose: T) -> windows_core::Result<Self>
                    where
                        T: windows_core::Compose,
                    {
                        Self::IButtonFactory(|this| unsafe {
                            let (derived__, base__) = windows_core::Compose::compose(compose);
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).CreateInstance)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(&derived__),
                                base__ as *mut _ as _,
                                &mut result__,
                            )
                            .ok()?;
                            let _ = &derived__;
                            windows_core::imp::Type::from_abi(result__)
                        })
                    }
                    pub fn Content(&self) -> windows_core::Result<windows_core::IInspectable> {
                        let this = &windows_core::Interface::cast::<IContentControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Content)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn SetContent<P0>(&self, value: P0) -> windows_core::Result<()>
                    where
                        P0: windows_core::Param<windows_core::IInspectable>,
                    {
                        let this = &windows_core::Interface::cast::<IContentControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetContent)(
                                windows_core::Interface::as_raw(this),
                                value.param().abi(),
                            )
                            .ok()
                        }
                    }
                    pub fn FontSize(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).FontSize)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetFontSize)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsEnabled(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsEnabled)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsEnabled(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsEnabled)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Padding(&self) -> windows_core::Result<super::Thickness> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Padding)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetPadding(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetPadding)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn Width(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Width)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Height(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Height)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn HorizontalAlignment(
                        &self,
                    ) -> windows_core::Result<super::HorizontalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHorizontalAlignment(
                        &self,
                        value: super::HorizontalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn VerticalAlignment(
                        &self,
                    ) -> windows_core::Result<super::VerticalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).VerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVerticalAlignment(
                        &self,
                        value: super::VerticalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Margin(&self) -> windows_core::Result<super::Thickness> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Margin)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMargin(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMargin)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Name)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetName(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetName)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn Opacity(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Opacity)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetOpacity)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Visibility(&self) -> windows_core::Result<super::Visibility> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Visibility)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVisibility(
                        &self,
                        value: super::Visibility,
                    ) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVisibility)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn KeyDown<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::Input::KeyRoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        let handler = <super::Input::KeyEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveKeyDown,
                            ))
                        }
                    }
                    fn IButtonFactory<R, F: FnOnce(&IButtonFactory) -> windows_core::Result<R>>(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<Button, IButtonFactory> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    fn IButtonStatics<R, F: FnOnce(&IButtonStatics) -> windows_core::Result<R>>(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<Button, IButtonStatics> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for Button {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, IButton>();
                }
                unsafe impl windows_core::Interface for Button {
                    type Vtable = <IButton as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID = <IButton as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for Button {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Button";
                }
                unsafe impl Send for Button {}
                unsafe impl Sync for Button {}
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct CheckBox(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    CheckBox,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    CheckBox,
                    Primitives::ToggleButton,
                    Primitives::ButtonBase,
                    ContentControl,
                    Control,
                    super::FrameworkElement,
                    super::UIElement,
                    super::DependencyObject
                );
                impl CheckBox {
                    pub fn Click<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::RoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<Primitives::IButtonBase>(self)?;
                        let handler = <super::RoutedEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).Click)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveClick,
                            ))
                        }
                    }
                    pub fn new() -> windows_core::Result<Self> {
                        Self::ICheckBoxFactory(|this| unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).CreateInstance)(
                                windows_core::Interface::as_raw(this),
                                core::ptr::null_mut(),
                                core::ptr::null_mut(),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        })
                    }
                    pub fn compose<T>(compose: T) -> windows_core::Result<Self>
                    where
                        T: windows_core::Compose,
                    {
                        Self::ICheckBoxFactory(|this| unsafe {
                            let (derived__, base__) = windows_core::Compose::compose(compose);
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).CreateInstance)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(&derived__),
                                base__ as *mut _ as _,
                                &mut result__,
                            )
                            .ok()?;
                            let _ = &derived__;
                            windows_core::imp::Type::from_abi(result__)
                        })
                    }
                    pub fn Content(&self) -> windows_core::Result<windows_core::IInspectable> {
                        let this = &windows_core::Interface::cast::<IContentControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Content)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn SetContent<P0>(&self, value: P0) -> windows_core::Result<()>
                    where
                        P0: windows_core::Param<windows_core::IInspectable>,
                    {
                        let this = &windows_core::Interface::cast::<IContentControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetContent)(
                                windows_core::Interface::as_raw(this),
                                value.param().abi(),
                            )
                            .ok()
                        }
                    }
                    pub fn FontSize(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).FontSize)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetFontSize)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsEnabled(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsEnabled)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsEnabled(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsEnabled)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Padding(&self) -> windows_core::Result<super::Thickness> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Padding)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetPadding(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetPadding)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn Width(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Width)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Height(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Height)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn HorizontalAlignment(
                        &self,
                    ) -> windows_core::Result<super::HorizontalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHorizontalAlignment(
                        &self,
                        value: super::HorizontalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn VerticalAlignment(
                        &self,
                    ) -> windows_core::Result<super::VerticalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).VerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVerticalAlignment(
                        &self,
                        value: super::VerticalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Margin(&self) -> windows_core::Result<super::Thickness> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Margin)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMargin(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMargin)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Name)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetName(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetName)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn IsChecked(&self) -> windows_core::Result<bool> {
                        let this =
                            &windows_core::Interface::cast::<Primitives::IToggleButton>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsChecked)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                            .and_then(|r__: windows_reference::IReference<bool>| r__.Value())
                        }
                    }
                    pub fn SetIsChecked(&self, value: Option<bool>) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<Primitives::IToggleButton>(self)?;
                        let value__ =
                            value.map(<windows_reference::IReference<bool> as From<_>>::from);
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsChecked)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Param::param(value__.as_ref()).abi(),
                            )
                            .ok()
                        }
                    }
                    pub fn IsThreeState(&self) -> windows_core::Result<bool> {
                        let this =
                            &windows_core::Interface::cast::<Primitives::IToggleButton>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsThreeState)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsThreeState(&self, value: bool) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<Primitives::IToggleButton>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsThreeState)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Checked<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::RoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this =
                            &windows_core::Interface::cast::<Primitives::IToggleButton>(self)?;
                        let handler = <super::RoutedEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).Checked)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveChecked,
                            ))
                        }
                    }
                    pub fn Unchecked<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::RoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this =
                            &windows_core::Interface::cast::<Primitives::IToggleButton>(self)?;
                        let handler = <super::RoutedEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).Unchecked)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveUnchecked,
                            ))
                        }
                    }
                    pub fn Opacity(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Opacity)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetOpacity)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Visibility(&self) -> windows_core::Result<super::Visibility> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Visibility)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVisibility(
                        &self,
                        value: super::Visibility,
                    ) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVisibility)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn KeyDown<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::Input::KeyRoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        let handler = <super::Input::KeyEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveKeyDown,
                            ))
                        }
                    }
                    fn ICheckBoxFactory<
                        R,
                        F: FnOnce(&ICheckBoxFactory) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<CheckBox, ICheckBoxFactory> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for CheckBox {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, ICheckBox>();
                }
                unsafe impl windows_core::Interface for CheckBox {
                    type Vtable = <ICheckBox as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID = <ICheckBox as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for CheckBox {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.CheckBox";
                }
                unsafe impl Send for CheckBox {}
                unsafe impl Sync for CheckBox {}
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct ColumnDefinition(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    ColumnDefinition,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(ColumnDefinition, super::DependencyObject);
                impl ColumnDefinition {
                    pub fn new() -> windows_core::Result<Self> {
                        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
                    }
                    fn IActivationFactory<
                        R,
                        F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            ColumnDefinition,
                            windows_core::imp::IGenericFactory,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    pub fn Width(&self) -> windows_core::Result<super::GridLength> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Width)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetWidth(&self, value: super::GridLength) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetWidth)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    fn IColumnDefinitionStatics<
                        R,
                        F: FnOnce(&IColumnDefinitionStatics) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            ColumnDefinition,
                            IColumnDefinitionStatics,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for ColumnDefinition {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, IColumnDefinition>();
                }
                unsafe impl windows_core::Interface for ColumnDefinition {
                    type Vtable = <IColumnDefinition as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <IColumnDefinition as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for ColumnDefinition {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ColumnDefinition";
                }
                unsafe impl Send for ColumnDefinition {}
                unsafe impl Sync for ColumnDefinition {}
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct ColumnDefinitionCollection(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    ColumnDefinitionCollection,
                    windows_core::IUnknown,
                    windows_core::IInspectable,
                    super::super::super::super::Windows::Foundation::Collections::IVector<
                        ColumnDefinition,
                    >
                );
                impl windows_core::RuntimeType for ColumnDefinitionCollection {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<
                            Self,
                            super::super::super::super::Windows::Foundation::Collections::IVector<
                                ColumnDefinition,
                            >,
                        >();
                }
                unsafe impl windows_core::Interface for ColumnDefinitionCollection {
                    type Vtable =
                        <super::super::super::super::Windows::Foundation::Collections::IVector<
                            ColumnDefinition,
                        > as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <super::super::super::super::Windows::Foundation::Collections::IVector<
                            ColumnDefinition,
                        > as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for ColumnDefinitionCollection {
                    const NAME: &'static str =
                        "Microsoft.UI.Xaml.Controls.ColumnDefinitionCollection";
                }
                unsafe impl Send for ColumnDefinitionCollection {}
                unsafe impl Sync for ColumnDefinitionCollection {}
                impl IntoIterator for ColumnDefinitionCollection {
                    type Item = ColumnDefinition;
                    type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                    fn into_iter(self) -> Self::IntoIter {
                        IntoIterator::into_iter(&self)
                    }
                }
                impl IntoIterator for &ColumnDefinitionCollection {
                    type Item = ColumnDefinition;
                    type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                    fn into_iter(self) -> Self::IntoIter {
                        windows_collections::BufferedIterator::new(self.First().unwrap())
                    }
                }
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct ContentControl(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    ContentControl,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    ContentControl,
                    Control,
                    super::FrameworkElement,
                    super::UIElement,
                    super::DependencyObject
                );
                impl ContentControl {
                    pub fn Content(&self) -> windows_core::Result<windows_core::IInspectable> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Content)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn SetContent<P0>(&self, value: P0) -> windows_core::Result<()>
                    where
                        P0: windows_core::Param<windows_core::IInspectable>,
                    {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetContent)(
                                windows_core::Interface::as_raw(self),
                                value.param().abi(),
                            )
                            .ok()
                        }
                    }
                    pub fn FontSize(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).FontSize)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetFontSize)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsEnabled(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsEnabled)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsEnabled(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsEnabled)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Padding(&self) -> windows_core::Result<super::Thickness> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Padding)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetPadding(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetPadding)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn Width(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Width)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Height(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Height)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn HorizontalAlignment(
                        &self,
                    ) -> windows_core::Result<super::HorizontalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHorizontalAlignment(
                        &self,
                        value: super::HorizontalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn VerticalAlignment(
                        &self,
                    ) -> windows_core::Result<super::VerticalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).VerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVerticalAlignment(
                        &self,
                        value: super::VerticalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Margin(&self) -> windows_core::Result<super::Thickness> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Margin)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMargin(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMargin)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Name)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetName(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetName)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn Opacity(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Opacity)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetOpacity)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Visibility(&self) -> windows_core::Result<super::Visibility> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Visibility)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVisibility(
                        &self,
                        value: super::Visibility,
                    ) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVisibility)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn KeyDown<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::Input::KeyRoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        let handler = <super::Input::KeyEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveKeyDown,
                            ))
                        }
                    }
                    fn IContentControlFactory<
                        R,
                        F: FnOnce(&IContentControlFactory) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            ContentControl,
                            IContentControlFactory,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    fn IContentControlStatics<
                        R,
                        F: FnOnce(&IContentControlStatics) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            ContentControl,
                            IContentControlStatics,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for ContentControl {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, IContentControl>();
                }
                unsafe impl windows_core::Interface for ContentControl {
                    type Vtable = <IContentControl as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <IContentControl as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for ContentControl {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ContentControl";
                }
                unsafe impl Send for ContentControl {}
                unsafe impl Sync for ContentControl {}
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct Control(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    Control,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    Control,
                    super::FrameworkElement,
                    super::UIElement,
                    super::DependencyObject
                );
                impl Control {
                    pub fn FontSize(&self) -> windows_core::Result<f64> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).FontSize)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetFontSize)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsEnabled(&self) -> windows_core::Result<bool> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).IsEnabled)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsEnabled(&self, value: bool) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetIsEnabled)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Padding(&self) -> windows_core::Result<super::Thickness> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Padding)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetPadding(&self, value: super::Thickness) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetPadding)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn Width(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Width)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Height(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Height)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn HorizontalAlignment(
                        &self,
                    ) -> windows_core::Result<super::HorizontalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHorizontalAlignment(
                        &self,
                        value: super::HorizontalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn VerticalAlignment(
                        &self,
                    ) -> windows_core::Result<super::VerticalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).VerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVerticalAlignment(
                        &self,
                        value: super::VerticalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Margin(&self) -> windows_core::Result<super::Thickness> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Margin)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMargin(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMargin)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Name)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetName(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetName)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn Opacity(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Opacity)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetOpacity)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Visibility(&self) -> windows_core::Result<super::Visibility> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Visibility)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVisibility(
                        &self,
                        value: super::Visibility,
                    ) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVisibility)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn KeyDown<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::Input::KeyRoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        let handler = <super::Input::KeyEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveKeyDown,
                            ))
                        }
                    }
                    fn IControlFactory<
                        R,
                        F: FnOnce(&IControlFactory) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<Control, IControlFactory> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    fn IControlStatics<
                        R,
                        F: FnOnce(&IControlStatics) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<Control, IControlStatics> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for Control {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, IControl>();
                }
                unsafe impl windows_core::Interface for Control {
                    type Vtable = <IControl as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID = <IControl as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for Control {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Control";
                }
                unsafe impl Send for Control {}
                unsafe impl Sync for Control {}
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct Grid(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    Grid,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    Grid,
                    Panel,
                    super::FrameworkElement,
                    super::UIElement,
                    super::DependencyObject
                );
                impl Grid {
                    pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn Width(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Width)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Height(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Height)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn HorizontalAlignment(
                        &self,
                    ) -> windows_core::Result<super::HorizontalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHorizontalAlignment(
                        &self,
                        value: super::HorizontalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn VerticalAlignment(
                        &self,
                    ) -> windows_core::Result<super::VerticalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).VerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVerticalAlignment(
                        &self,
                        value: super::VerticalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Margin(&self) -> windows_core::Result<super::Thickness> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Margin)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMargin(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMargin)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Name)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetName(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetName)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn RowDefinitions(&self) -> windows_core::Result<RowDefinitionCollection> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).RowDefinitions)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn ColumnDefinitions(
                        &self,
                    ) -> windows_core::Result<ColumnDefinitionCollection> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).ColumnDefinitions)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn Padding(&self) -> windows_core::Result<super::Thickness> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Padding)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetPadding(&self, value: super::Thickness) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetPadding)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn RowSpacing(&self) -> windows_core::Result<f64> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).RowSpacing)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetRowSpacing(&self, value: f64) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetRowSpacing)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn ColumnSpacing(&self) -> windows_core::Result<f64> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).ColumnSpacing)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetColumnSpacing(&self, value: f64) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetColumnSpacing)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn new() -> windows_core::Result<Self> {
                        Self::IGridFactory(|this| unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).CreateInstance)(
                                windows_core::Interface::as_raw(this),
                                core::ptr::null_mut(),
                                core::ptr::null_mut(),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        })
                    }
                    pub fn compose<T>(compose: T) -> windows_core::Result<Self>
                    where
                        T: windows_core::Compose,
                    {
                        Self::IGridFactory(|this| unsafe {
                            let (derived__, base__) = windows_core::Compose::compose(compose);
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).CreateInstance)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(&derived__),
                                base__ as *mut _ as _,
                                &mut result__,
                            )
                            .ok()?;
                            let _ = &derived__;
                            windows_core::imp::Type::from_abi(result__)
                        })
                    }
                    pub fn GetRow<P0>(element: P0) -> windows_core::Result<i32>
                    where
                        P0: windows_core::Param<super::FrameworkElement>,
                    {
                        Self::IGridStatics(|this| unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).GetRow)(
                                windows_core::Interface::as_raw(this),
                                element.param().abi(),
                                &mut result__,
                            )
                            .map(|| result__)
                        })
                    }
                    pub fn SetRow<P0>(element: P0, value: i32) -> windows_core::Result<()>
                    where
                        P0: windows_core::Param<super::FrameworkElement>,
                    {
                        Self::IGridStatics(|this| unsafe {
                            (windows_core::Interface::vtable(this).SetRow)(
                                windows_core::Interface::as_raw(this),
                                element.param().abi(),
                                value,
                            )
                            .ok()
                        })
                    }
                    pub fn GetColumn<P0>(element: P0) -> windows_core::Result<i32>
                    where
                        P0: windows_core::Param<super::FrameworkElement>,
                    {
                        Self::IGridStatics(|this| unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).GetColumn)(
                                windows_core::Interface::as_raw(this),
                                element.param().abi(),
                                &mut result__,
                            )
                            .map(|| result__)
                        })
                    }
                    pub fn SetColumn<P0>(element: P0, value: i32) -> windows_core::Result<()>
                    where
                        P0: windows_core::Param<super::FrameworkElement>,
                    {
                        Self::IGridStatics(|this| unsafe {
                            (windows_core::Interface::vtable(this).SetColumn)(
                                windows_core::Interface::as_raw(this),
                                element.param().abi(),
                                value,
                            )
                            .ok()
                        })
                    }
                    pub fn GetRowSpan<P0>(element: P0) -> windows_core::Result<i32>
                    where
                        P0: windows_core::Param<super::FrameworkElement>,
                    {
                        Self::IGridStatics(|this| unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).GetRowSpan)(
                                windows_core::Interface::as_raw(this),
                                element.param().abi(),
                                &mut result__,
                            )
                            .map(|| result__)
                        })
                    }
                    pub fn SetRowSpan<P0>(element: P0, value: i32) -> windows_core::Result<()>
                    where
                        P0: windows_core::Param<super::FrameworkElement>,
                    {
                        Self::IGridStatics(|this| unsafe {
                            (windows_core::Interface::vtable(this).SetRowSpan)(
                                windows_core::Interface::as_raw(this),
                                element.param().abi(),
                                value,
                            )
                            .ok()
                        })
                    }
                    pub fn GetColumnSpan<P0>(element: P0) -> windows_core::Result<i32>
                    where
                        P0: windows_core::Param<super::FrameworkElement>,
                    {
                        Self::IGridStatics(|this| unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).GetColumnSpan)(
                                windows_core::Interface::as_raw(this),
                                element.param().abi(),
                                &mut result__,
                            )
                            .map(|| result__)
                        })
                    }
                    pub fn SetColumnSpan<P0>(element: P0, value: i32) -> windows_core::Result<()>
                    where
                        P0: windows_core::Param<super::FrameworkElement>,
                    {
                        Self::IGridStatics(|this| unsafe {
                            (windows_core::Interface::vtable(this).SetColumnSpan)(
                                windows_core::Interface::as_raw(this),
                                element.param().abi(),
                                value,
                            )
                            .ok()
                        })
                    }
                    pub fn Children(&self) -> windows_core::Result<UIElementCollection> {
                        let this = &windows_core::Interface::cast::<IPanel>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Children)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn Opacity(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Opacity)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetOpacity)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Visibility(&self) -> windows_core::Result<super::Visibility> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Visibility)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVisibility(
                        &self,
                        value: super::Visibility,
                    ) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVisibility)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn KeyDown<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::Input::KeyRoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        let handler = <super::Input::KeyEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveKeyDown,
                            ))
                        }
                    }
                    fn IGridFactory<R, F: FnOnce(&IGridFactory) -> windows_core::Result<R>>(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<Grid, IGridFactory> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    fn IGridStatics<R, F: FnOnce(&IGridStatics) -> windows_core::Result<R>>(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<Grid, IGridStatics> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for Grid {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, IGrid>();
                }
                unsafe impl windows_core::Interface for Grid {
                    type Vtable = <IGrid as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID = <IGrid as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for Grid {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Grid";
                }
                unsafe impl Send for Grid {}
                unsafe impl Sync for Grid {}
                windows_core::imp::define_interface!(
                    IButton,
                    IButton_Vtbl,
                    0x216c183d_d07a_5aa5_b8a4_0300a2683e87
                );
                impl windows_core::RuntimeType for IButton {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IButton",
                        );
                }
                #[repr(C)]
                pub struct IButton_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    IButtonFactory,
                    IButtonFactory_Vtbl,
                    0xfe393422_d91c_57b1_9a9c_2c7e3f41f77c
                );
                impl windows_core::RuntimeType for IButtonFactory {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IButtonFactory",
                        );
                }
                #[repr(C)]
                pub struct IButtonFactory_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub CreateInstance: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IButtonStatics,
                    IButtonStatics_Vtbl,
                    0x57823d25_b26a_5e0f_94f6_bbae70683dc5
                );
                impl windows_core::RuntimeType for IButtonStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IButtonStatics",
                        );
                }
                #[repr(C)]
                pub struct IButtonStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    ICheckBox,
                    ICheckBox_Vtbl,
                    0xc5830000_4c9d_5fdd_9346_674c71cd80c5
                );
                impl windows_core::RuntimeType for ICheckBox {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.ICheckBox",
                        );
                }
                #[repr(C)]
                pub struct ICheckBox_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    ICheckBoxFactory,
                    ICheckBoxFactory_Vtbl,
                    0xf43ff58d_31d5_5835_af7b_375bc6a9bcf3
                );
                impl windows_core::RuntimeType for ICheckBoxFactory {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.ICheckBoxFactory",
                        );
                }
                #[repr(C)]
                pub struct ICheckBoxFactory_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub CreateInstance: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IColumnDefinition,
                    IColumnDefinition_Vtbl,
                    0x454cea14_87ec_5890_bb62_f1d82a94758e
                );
                impl windows_core::RuntimeType for IColumnDefinition {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IColumnDefinition",
                        );
                }
                #[repr(C)]
                pub struct IColumnDefinition_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub Width: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut super::GridLength,
                    )
                        -> windows_core::HRESULT,
                    pub SetWidth: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        super::GridLength,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IColumnDefinitionStatics,
                    IColumnDefinitionStatics_Vtbl,
                    0xaba4b755_ca63_5ed7_ae31_45f9f91920b0
                );
                impl windows_core::RuntimeType for IColumnDefinitionStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IColumnDefinitionStatics",
                        );
                }
                #[repr(C)]
                pub struct IColumnDefinitionStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    IContentControl,
                    IContentControl_Vtbl,
                    0x07e81761_11b2_52ae_8f8b_4d53d2b5900a
                );
                impl windows_core::RuntimeType for IContentControl {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IContentControl",
                        );
                }
                #[repr(C)]
                pub struct IContentControl_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub Content: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    pub SetContent: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IContentControlFactory,
                    IContentControlFactory_Vtbl,
                    0x3dea958e_5acd_5f80_8938_38634f51493a
                );
                impl windows_core::RuntimeType for IContentControlFactory {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IContentControlFactory",
                        );
                }
                #[repr(C)]
                pub struct IContentControlFactory_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    IContentControlStatics,
                    IContentControlStatics_Vtbl,
                    0xf25484f4_2fed_5a0a_8864_7d6d4ac43ef8
                );
                impl windows_core::RuntimeType for IContentControlStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IContentControlStatics",
                        );
                }
                #[repr(C)]
                pub struct IContentControlStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    IControl,
                    IControl_Vtbl,
                    0x857d6e8a_d45a_5c69_a99c_bf6a5c54fb38
                );
                impl windows_core::RuntimeType for IControl {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IControl",
                        );
                }
                #[repr(C)]
                pub struct IControl_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    IsFocusEngagementEnabled: usize,
                    SetIsFocusEngagementEnabled: usize,
                    IsFocusEngaged: usize,
                    SetIsFocusEngaged: usize,
                    RequiresPointer: usize,
                    SetRequiresPointer: usize,
                    pub FontSize: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut f64,
                    )
                        -> windows_core::HRESULT,
                    pub SetFontSize: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        f64,
                    )
                        -> windows_core::HRESULT,
                    FontFamily: usize,
                    SetFontFamily: usize,
                    FontWeight: usize,
                    SetFontWeight: usize,
                    FontStyle: usize,
                    SetFontStyle: usize,
                    FontStretch: usize,
                    SetFontStretch: usize,
                    CharacterSpacing: usize,
                    SetCharacterSpacing: usize,
                    Foreground: usize,
                    SetForeground: usize,
                    IsTextScaleFactorEnabled: usize,
                    SetIsTextScaleFactorEnabled: usize,
                    pub IsEnabled: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut bool,
                    )
                        -> windows_core::HRESULT,
                    pub SetIsEnabled: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        bool,
                    )
                        -> windows_core::HRESULT,
                    TabNavigation: usize,
                    SetTabNavigation: usize,
                    Template: usize,
                    SetTemplate: usize,
                    pub Padding: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut super::Thickness,
                    )
                        -> windows_core::HRESULT,
                    pub SetPadding: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        super::Thickness,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IControlFactory,
                    IControlFactory_Vtbl,
                    0x25159233_9438_5534_aeb9_00eb059cf73f
                );
                impl windows_core::RuntimeType for IControlFactory {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IControlFactory",
                        );
                }
                #[repr(C)]
                pub struct IControlFactory_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    IControlStatics,
                    IControlStatics_Vtbl,
                    0xc3ae388d_aa36_5e10_acac_98415f47bcc7
                );
                impl windows_core::RuntimeType for IControlStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IControlStatics",
                        );
                }
                #[repr(C)]
                pub struct IControlStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    IGrid,
                    IGrid_Vtbl,
                    0xc4496219_9014_58a1_b4ad_c5044913a5bb
                );
                impl windows_core::RuntimeType for IGrid {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IGrid",
                        );
                }
                #[repr(C)]
                pub struct IGrid_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub RowDefinitions: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    pub ColumnDefinitions: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    BackgroundSizing: usize,
                    SetBackgroundSizing: usize,
                    BorderBrush: usize,
                    SetBorderBrush: usize,
                    BorderThickness: usize,
                    SetBorderThickness: usize,
                    CornerRadius: usize,
                    SetCornerRadius: usize,
                    pub Padding: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut super::Thickness,
                    )
                        -> windows_core::HRESULT,
                    pub SetPadding: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        super::Thickness,
                    )
                        -> windows_core::HRESULT,
                    pub RowSpacing: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut f64,
                    )
                        -> windows_core::HRESULT,
                    pub SetRowSpacing: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        f64,
                    )
                        -> windows_core::HRESULT,
                    pub ColumnSpacing: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut f64,
                    )
                        -> windows_core::HRESULT,
                    pub SetColumnSpacing: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        f64,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IGridFactory,
                    IGridFactory_Vtbl,
                    0xb16bf561_fc6c_57c6_8ebc_0b06ce4513aa
                );
                impl windows_core::RuntimeType for IGridFactory {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IGridFactory",
                        );
                }
                #[repr(C)]
                pub struct IGridFactory_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub CreateInstance: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IGridStatics,
                    IGridStatics_Vtbl,
                    0xef9cf81d_a431_50f4_abf5_3023fe447704
                );
                impl windows_core::RuntimeType for IGridStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IGridStatics",
                        );
                }
                #[repr(C)]
                pub struct IGridStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    BackgroundSizingProperty: usize,
                    BorderBrushProperty: usize,
                    BorderThicknessProperty: usize,
                    CornerRadiusProperty: usize,
                    PaddingProperty: usize,
                    RowSpacingProperty: usize,
                    ColumnSpacingProperty: usize,
                    RowProperty: usize,
                    pub GetRow: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut i32,
                    )
                        -> windows_core::HRESULT,
                    pub SetRow: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        i32,
                    )
                        -> windows_core::HRESULT,
                    ColumnProperty: usize,
                    pub GetColumn: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut i32,
                    )
                        -> windows_core::HRESULT,
                    pub SetColumn: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        i32,
                    )
                        -> windows_core::HRESULT,
                    RowSpanProperty: usize,
                    pub GetRowSpan: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut i32,
                    )
                        -> windows_core::HRESULT,
                    pub SetRowSpan: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        i32,
                    )
                        -> windows_core::HRESULT,
                    ColumnSpanProperty: usize,
                    pub GetColumnSpan: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut i32,
                    )
                        -> windows_core::HRESULT,
                    pub SetColumnSpan: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        i32,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IPanel,
                    IPanel_Vtbl,
                    0x27a1b418_56f3_525e_b883_cefed905eed3
                );
                impl windows_core::RuntimeType for IPanel {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IPanel",
                        );
                }
                #[repr(C)]
                pub struct IPanel_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub Children: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IPanelFactory,
                    IPanelFactory_Vtbl,
                    0xf5e7e21c_4c97_5d20_bee6_3e4fc6ab14e9
                );
                impl windows_core::RuntimeType for IPanelFactory {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IPanelFactory",
                        );
                }
                #[repr(C)]
                pub struct IPanelFactory_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    IPanelStatics,
                    IPanelStatics_Vtbl,
                    0x76a9caa7_a5d4_5061_a325_17c76f66de51
                );
                impl windows_core::RuntimeType for IPanelStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IPanelStatics",
                        );
                }
                #[repr(C)]
                pub struct IPanelStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    IRowDefinition,
                    IRowDefinition_Vtbl,
                    0xfe870f2f_89ef_5dac_9f33_968d0dc577c3
                );
                impl windows_core::RuntimeType for IRowDefinition {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IRowDefinition",
                        );
                }
                #[repr(C)]
                pub struct IRowDefinition_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub Height: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut super::GridLength,
                    )
                        -> windows_core::HRESULT,
                    pub SetHeight: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        super::GridLength,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IRowDefinitionStatics,
                    IRowDefinitionStatics_Vtbl,
                    0x7317c7c5_fd6f_58d9_9b53_4978dbacde23
                );
                impl windows_core::RuntimeType for IRowDefinitionStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IRowDefinitionStatics",
                        );
                }
                #[repr(C)]
                pub struct IRowDefinitionStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    ISlider,
                    ISlider_Vtbl,
                    0xf7418ecf_7c35_5216_8bf1_d82d47cce5df
                );
                impl windows_core::RuntimeType for ISlider {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.ISlider",
                        );
                }
                #[repr(C)]
                pub struct ISlider_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    IntermediateValue: usize,
                    SetIntermediateValue: usize,
                    pub StepFrequency: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut f64,
                    )
                        -> windows_core::HRESULT,
                    pub SetStepFrequency: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        f64,
                    )
                        -> windows_core::HRESULT,
                    SnapsTo: usize,
                    SetSnapsTo: usize,
                    TickFrequency: usize,
                    SetTickFrequency: usize,
                    TickPlacement: usize,
                    SetTickPlacement: usize,
                    pub Orientation: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut Orientation,
                    )
                        -> windows_core::HRESULT,
                    pub SetOrientation: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        Orientation,
                    )
                        -> windows_core::HRESULT,
                    IsDirectionReversed: usize,
                    SetIsDirectionReversed: usize,
                    IsThumbToolTipEnabled: usize,
                    SetIsThumbToolTipEnabled: usize,
                    ThumbToolTipValueConverter: usize,
                    SetThumbToolTipValueConverter: usize,
                    pub Header: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    pub SetHeader: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    ISliderFactory,
                    ISliderFactory_Vtbl,
                    0x06604d71_34ca_5f39_9656_29d81d3c110c
                );
                impl windows_core::RuntimeType for ISliderFactory {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.ISliderFactory",
                        );
                }
                #[repr(C)]
                pub struct ISliderFactory_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub CreateInstance: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    ISliderStatics,
                    ISliderStatics_Vtbl,
                    0xf0541f7b_6d81_59e2_997a_2e65fbcccd21
                );
                impl windows_core::RuntimeType for ISliderStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.ISliderStatics",
                        );
                }
                #[repr(C)]
                pub struct ISliderStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    IStackPanel,
                    IStackPanel_Vtbl,
                    0x493ab00b_3a6a_5e4a_9452_407cd5197406
                );
                impl windows_core::RuntimeType for IStackPanel {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IStackPanel",
                        );
                }
                #[repr(C)]
                pub struct IStackPanel_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    AreScrollSnapPointsRegular: usize,
                    SetAreScrollSnapPointsRegular: usize,
                    pub Orientation: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut Orientation,
                    )
                        -> windows_core::HRESULT,
                    pub SetOrientation: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        Orientation,
                    )
                        -> windows_core::HRESULT,
                    BackgroundSizing: usize,
                    SetBackgroundSizing: usize,
                    BorderBrush: usize,
                    SetBorderBrush: usize,
                    BorderThickness: usize,
                    SetBorderThickness: usize,
                    CornerRadius: usize,
                    SetCornerRadius: usize,
                    pub Padding: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut super::Thickness,
                    )
                        -> windows_core::HRESULT,
                    pub SetPadding: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        super::Thickness,
                    )
                        -> windows_core::HRESULT,
                    pub Spacing: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut f64,
                    )
                        -> windows_core::HRESULT,
                    pub SetSpacing: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        f64,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IStackPanelFactory,
                    IStackPanelFactory_Vtbl,
                    0x64c1d388_47a2_5a74_a75b_559d151ee5ac
                );
                impl windows_core::RuntimeType for IStackPanelFactory {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IStackPanelFactory",
                        );
                }
                #[repr(C)]
                pub struct IStackPanelFactory_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub CreateInstance: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IStackPanelStatics,
                    IStackPanelStatics_Vtbl,
                    0x10bb04e3_eb01_5ea8_9f96_69508479def9
                );
                impl windows_core::RuntimeType for IStackPanelStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IStackPanelStatics",
                        );
                }
                #[repr(C)]
                pub struct IStackPanelStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    ITextBlock,
                    ITextBlock_Vtbl,
                    0x1ac8d84f_392c_5c7e_83f5_a53e3bf0abb0
                );
                impl windows_core::RuntimeType for ITextBlock {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.ITextBlock",
                        );
                }
                #[repr(C)]
                pub struct ITextBlock_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub FontSize: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut f64,
                    )
                        -> windows_core::HRESULT,
                    pub SetFontSize: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        f64,
                    )
                        -> windows_core::HRESULT,
                    FontFamily: usize,
                    SetFontFamily: usize,
                    FontWeight: usize,
                    SetFontWeight: usize,
                    FontStyle: usize,
                    SetFontStyle: usize,
                    FontStretch: usize,
                    SetFontStretch: usize,
                    CharacterSpacing: usize,
                    SetCharacterSpacing: usize,
                    Foreground: usize,
                    SetForeground: usize,
                    pub TextWrapping: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut super::TextWrapping,
                    )
                        -> windows_core::HRESULT,
                    pub SetTextWrapping: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        super::TextWrapping,
                    )
                        -> windows_core::HRESULT,
                    TextTrimming: usize,
                    SetTextTrimming: usize,
                    TextAlignment: usize,
                    SetTextAlignment: usize,
                    pub Text: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    pub SetText: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    Inlines: usize,
                    Padding: usize,
                    SetPadding: usize,
                    LineHeight: usize,
                    SetLineHeight: usize,
                    LineStackingStrategy: usize,
                    SetLineStackingStrategy: usize,
                    pub IsTextSelectionEnabled: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut bool,
                    )
                        -> windows_core::HRESULT,
                    pub SetIsTextSelectionEnabled:
                        unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            bool,
                        ) -> windows_core::HRESULT,
                    SelectedText: usize,
                    ContentStart: usize,
                    ContentEnd: usize,
                    SelectionStart: usize,
                    SelectionEnd: usize,
                    BaselineOffset: usize,
                    SelectionHighlightColor: usize,
                    SetSelectionHighlightColor: usize,
                    pub MaxLines: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut i32,
                    )
                        -> windows_core::HRESULT,
                    pub SetMaxLines: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        i32,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    ITextBlockStatics,
                    ITextBlockStatics_Vtbl,
                    0x3187104b_65c2_5e53_b889_c8272b1314cd
                );
                impl windows_core::RuntimeType for ITextBlockStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.ITextBlockStatics",
                        );
                }
                #[repr(C)]
                pub struct ITextBlockStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    ITextBox,
                    ITextBox_Vtbl,
                    0x873af7c2_ab89_5d76_8dbe_3d6325669df5
                );
                impl windows_core::RuntimeType for ITextBox {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.ITextBox",
                        );
                }
                #[repr(C)]
                pub struct ITextBox_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub Text: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    pub SetText: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    SelectedText: usize,
                    SetSelectedText: usize,
                    SelectionLength: usize,
                    SetSelectionLength: usize,
                    SelectionStart: usize,
                    SetSelectionStart: usize,
                    pub MaxLength: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut i32,
                    )
                        -> windows_core::HRESULT,
                    pub SetMaxLength: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        i32,
                    )
                        -> windows_core::HRESULT,
                    pub IsReadOnly: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut bool,
                    )
                        -> windows_core::HRESULT,
                    pub SetIsReadOnly: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        bool,
                    )
                        -> windows_core::HRESULT,
                    pub AcceptsReturn: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut bool,
                    )
                        -> windows_core::HRESULT,
                    pub SetAcceptsReturn: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        bool,
                    )
                        -> windows_core::HRESULT,
                    TextAlignment: usize,
                    SetTextAlignment: usize,
                    pub TextWrapping: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut super::TextWrapping,
                    )
                        -> windows_core::HRESULT,
                    pub SetTextWrapping: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        super::TextWrapping,
                    )
                        -> windows_core::HRESULT,
                    IsSpellCheckEnabled: usize,
                    SetIsSpellCheckEnabled: usize,
                    IsTextPredictionEnabled: usize,
                    SetIsTextPredictionEnabled: usize,
                    InputScope: usize,
                    SetInputScope: usize,
                    pub Header: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    pub SetHeader: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    HeaderTemplate: usize,
                    SetHeaderTemplate: usize,
                    pub PlaceholderText: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    pub SetPlaceholderText: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    SelectionHighlightColor: usize,
                    SetSelectionHighlightColor: usize,
                    PreventKeyboardDisplayOnProgrammaticFocus: usize,
                    SetPreventKeyboardDisplayOnProgrammaticFocus: usize,
                    IsColorFontEnabled: usize,
                    SetIsColorFontEnabled: usize,
                    SelectionHighlightColorWhenNotFocused: usize,
                    SetSelectionHighlightColorWhenNotFocused: usize,
                    HorizontalTextAlignment: usize,
                    SetHorizontalTextAlignment: usize,
                    CharacterCasing: usize,
                    SetCharacterCasing: usize,
                    PlaceholderForeground: usize,
                    SetPlaceholderForeground: usize,
                    CanPasteClipboardContent: usize,
                    CanUndo: usize,
                    CanRedo: usize,
                    SelectionFlyout: usize,
                    SetSelectionFlyout: usize,
                    ProofingMenuFlyout: usize,
                    Description: usize,
                    SetDescription: usize,
                    pub TextChanged: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut i64,
                    )
                        -> windows_core::HRESULT,
                    pub RemoveTextChanged: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        i64,
                    )
                        -> windows_core::HRESULT,
                    SelectionChanged: usize,
                    RemoveSelectionChanged: usize,
                    ContextMenuOpening: usize,
                    RemoveContextMenuOpening: usize,
                    Paste: usize,
                    RemovePaste: usize,
                    TextCompositionStarted: usize,
                    RemoveTextCompositionStarted: usize,
                    TextCompositionChanged: usize,
                    RemoveTextCompositionChanged: usize,
                    TextCompositionEnded: usize,
                    RemoveTextCompositionEnded: usize,
                    CopyingToClipboard: usize,
                    RemoveCopyingToClipboard: usize,
                    CuttingToClipboard: usize,
                    RemoveCuttingToClipboard: usize,
                    pub BeforeTextChanging: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut i64,
                    )
                        -> windows_core::HRESULT,
                    pub RemoveBeforeTextChanging:
                        unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            i64,
                        ) -> windows_core::HRESULT,
                    SelectionChanging: usize,
                    RemoveSelectionChanging: usize,
                    pub Select: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        i32,
                        i32,
                    )
                        -> windows_core::HRESULT,
                    pub SelectAll:
                        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    ITextBoxBeforeTextChangingEventArgs,
                    ITextBoxBeforeTextChangingEventArgs_Vtbl,
                    0xba46105b_9a16_5237_a20f_5df750992a05
                );
                impl windows_core::RuntimeType for ITextBoxBeforeTextChangingEventArgs {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.ITextBoxBeforeTextChangingEventArgs",
                        );
                }
                #[repr(C)]
                pub struct ITextBoxBeforeTextChangingEventArgs_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub NewText: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    pub Cancel: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut bool,
                    )
                        -> windows_core::HRESULT,
                    pub SetCancel: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        bool,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    ITextBoxFactory,
                    ITextBoxFactory_Vtbl,
                    0xe1d8b82e_bc60_5d27_b646_5ca4c4a69432
                );
                impl windows_core::RuntimeType for ITextBoxFactory {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.ITextBoxFactory",
                        );
                }
                #[repr(C)]
                pub struct ITextBoxFactory_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub CreateInstance: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    ITextBoxStatics,
                    ITextBoxStatics_Vtbl,
                    0xa41cf38f_712a_5599_bbed_5a3d9b6bd46e
                );
                impl windows_core::RuntimeType for ITextBoxStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.ITextBoxStatics",
                        );
                }
                #[repr(C)]
                pub struct ITextBoxStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    ITextChangedEventArgs,
                    ITextChangedEventArgs_Vtbl,
                    0x71c37e43_7be7_52fc_bf8c_9867f44be5f4
                );
                impl windows_core::RuntimeType for ITextChangedEventArgs {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.ITextChangedEventArgs",
                        );
                }
                #[repr(C)]
                pub struct ITextChangedEventArgs_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    IXamlControlsResources,
                    IXamlControlsResources_Vtbl,
                    0x918ca043_f42c_5805_861b_62d6d1d0c162
                );
                impl windows_core::RuntimeType for IXamlControlsResources {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IXamlControlsResources",
                        );
                }
                #[repr(C)]
                pub struct IXamlControlsResources_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    IXamlControlsResourcesStatics,
                    IXamlControlsResourcesStatics_Vtbl,
                    0xcb49196c_1458_5e92_b701_d08d3e816bc5
                );
                impl windows_core::RuntimeType for IXamlControlsResourcesStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.IXamlControlsResourcesStatics",
                        );
                }
                #[repr(C)]
                pub struct IXamlControlsResourcesStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                #[repr(transparent)]
                #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
                pub struct Orientation(pub i32);
                impl Orientation {
                    pub const Vertical: Self = Self(0);
                    pub const Horizontal: Self = Self(1);
                }
                impl windows_core::imp::TypeKind for Orientation {
                    type TypeKind = windows_core::imp::CopyType;
                }
                impl windows_core::RuntimeType for Orientation {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"enum(Microsoft.UI.Xaml.Controls.Orientation;i4)",
                        );
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Controls.Orientation",
                        );
                }
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct Panel(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    Panel,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    Panel,
                    super::FrameworkElement,
                    super::UIElement,
                    super::DependencyObject
                );
                impl Panel {
                    pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn Width(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Width)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Height(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Height)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn HorizontalAlignment(
                        &self,
                    ) -> windows_core::Result<super::HorizontalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHorizontalAlignment(
                        &self,
                        value: super::HorizontalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn VerticalAlignment(
                        &self,
                    ) -> windows_core::Result<super::VerticalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).VerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVerticalAlignment(
                        &self,
                        value: super::VerticalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Margin(&self) -> windows_core::Result<super::Thickness> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Margin)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMargin(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMargin)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Name)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetName(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetName)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn Children(&self) -> windows_core::Result<UIElementCollection> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Children)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn Opacity(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Opacity)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetOpacity)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Visibility(&self) -> windows_core::Result<super::Visibility> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Visibility)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVisibility(
                        &self,
                        value: super::Visibility,
                    ) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVisibility)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn KeyDown<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::Input::KeyRoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        let handler = <super::Input::KeyEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveKeyDown,
                            ))
                        }
                    }
                    fn IPanelFactory<R, F: FnOnce(&IPanelFactory) -> windows_core::Result<R>>(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<Panel, IPanelFactory> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    fn IPanelStatics<R, F: FnOnce(&IPanelStatics) -> windows_core::Result<R>>(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<Panel, IPanelStatics> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for Panel {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, IPanel>();
                }
                unsafe impl windows_core::Interface for Panel {
                    type Vtable = <IPanel as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID = <IPanel as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for Panel {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Panel";
                }
                unsafe impl Send for Panel {}
                unsafe impl Sync for Panel {}
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct RowDefinition(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    RowDefinition,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(RowDefinition, super::DependencyObject);
                impl RowDefinition {
                    pub fn new() -> windows_core::Result<Self> {
                        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
                    }
                    fn IActivationFactory<
                        R,
                        F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            RowDefinition,
                            windows_core::imp::IGenericFactory,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    pub fn Height(&self) -> windows_core::Result<super::GridLength> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Height)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHeight(&self, value: super::GridLength) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetHeight)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    fn IRowDefinitionStatics<
                        R,
                        F: FnOnce(&IRowDefinitionStatics) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            RowDefinition,
                            IRowDefinitionStatics,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for RowDefinition {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, IRowDefinition>();
                }
                unsafe impl windows_core::Interface for RowDefinition {
                    type Vtable = <IRowDefinition as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <IRowDefinition as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for RowDefinition {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.RowDefinition";
                }
                unsafe impl Send for RowDefinition {}
                unsafe impl Sync for RowDefinition {}
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct RowDefinitionCollection(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    RowDefinitionCollection,
                    windows_core::IUnknown,
                    windows_core::IInspectable,
                    super::super::super::super::Windows::Foundation::Collections::IVector<
                        RowDefinition,
                    >
                );
                impl windows_core::RuntimeType for RowDefinitionCollection {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<
                            Self,
                            super::super::super::super::Windows::Foundation::Collections::IVector<
                                RowDefinition,
                            >,
                        >();
                }
                unsafe impl windows_core::Interface for RowDefinitionCollection {
                    type Vtable =
                        <super::super::super::super::Windows::Foundation::Collections::IVector<
                            RowDefinition,
                        > as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <super::super::super::super::Windows::Foundation::Collections::IVector<
                            RowDefinition,
                        > as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for RowDefinitionCollection {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.RowDefinitionCollection";
                }
                unsafe impl Send for RowDefinitionCollection {}
                unsafe impl Sync for RowDefinitionCollection {}
                impl IntoIterator for RowDefinitionCollection {
                    type Item = RowDefinition;
                    type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                    fn into_iter(self) -> Self::IntoIter {
                        IntoIterator::into_iter(&self)
                    }
                }
                impl IntoIterator for &RowDefinitionCollection {
                    type Item = RowDefinition;
                    type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                    fn into_iter(self) -> Self::IntoIter {
                        windows_collections::BufferedIterator::new(self.First().unwrap())
                    }
                }
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct Slider(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    Slider,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    Slider,
                    Primitives::RangeBase,
                    Control,
                    super::FrameworkElement,
                    super::UIElement,
                    super::DependencyObject
                );
                impl Slider {
                    pub fn FontSize(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).FontSize)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetFontSize)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsEnabled(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsEnabled)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsEnabled(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsEnabled)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Padding(&self) -> windows_core::Result<super::Thickness> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Padding)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetPadding(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetPadding)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn Width(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Width)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Height(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Height)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn HorizontalAlignment(
                        &self,
                    ) -> windows_core::Result<super::HorizontalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHorizontalAlignment(
                        &self,
                        value: super::HorizontalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn VerticalAlignment(
                        &self,
                    ) -> windows_core::Result<super::VerticalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).VerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVerticalAlignment(
                        &self,
                        value: super::VerticalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Margin(&self) -> windows_core::Result<super::Thickness> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Margin)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMargin(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMargin)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Name)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetName(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetName)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn Minimum(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<Primitives::IRangeBase>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Minimum)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinimum(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<Primitives::IRangeBase>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinimum)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Maximum(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<Primitives::IRangeBase>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Maximum)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMaximum(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<Primitives::IRangeBase>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMaximum)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn SmallChange(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<Primitives::IRangeBase>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).SmallChange)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetSmallChange(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<Primitives::IRangeBase>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetSmallChange)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn LargeChange(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<Primitives::IRangeBase>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).LargeChange)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetLargeChange(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<Primitives::IRangeBase>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetLargeChange)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Value(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<Primitives::IRangeBase>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Value)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetValue(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<Primitives::IRangeBase>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetValue)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn ValueChanged<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<Primitives::RangeBaseValueChangedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<Primitives::IRangeBase>(self)?;
                        let handler =
                            <Primitives::RangeBaseValueChangedEventHandler>::new(move |a0, a1| {
                                handler(a0, a1);
                                Ok(())
                            });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).ValueChanged)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveValueChanged,
                            ))
                        }
                    }
                    pub fn StepFrequency(&self) -> windows_core::Result<f64> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).StepFrequency)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetStepFrequency(&self, value: f64) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetStepFrequency)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Orientation(&self) -> windows_core::Result<Orientation> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Orientation)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetOrientation(&self, value: Orientation) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetOrientation)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Header(&self) -> windows_core::Result<windows_core::IInspectable> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Header)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn SetHeader<P0>(&self, value: P0) -> windows_core::Result<()>
                    where
                        P0: windows_core::Param<windows_core::IInspectable>,
                    {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetHeader)(
                                windows_core::Interface::as_raw(self),
                                value.param().abi(),
                            )
                            .ok()
                        }
                    }
                    pub fn new() -> windows_core::Result<Self> {
                        Self::ISliderFactory(|this| unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).CreateInstance)(
                                windows_core::Interface::as_raw(this),
                                core::ptr::null_mut(),
                                core::ptr::null_mut(),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        })
                    }
                    pub fn compose<T>(compose: T) -> windows_core::Result<Self>
                    where
                        T: windows_core::Compose,
                    {
                        Self::ISliderFactory(|this| unsafe {
                            let (derived__, base__) = windows_core::Compose::compose(compose);
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).CreateInstance)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(&derived__),
                                base__ as *mut _ as _,
                                &mut result__,
                            )
                            .ok()?;
                            let _ = &derived__;
                            windows_core::imp::Type::from_abi(result__)
                        })
                    }
                    pub fn Opacity(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Opacity)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetOpacity)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Visibility(&self) -> windows_core::Result<super::Visibility> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Visibility)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVisibility(
                        &self,
                        value: super::Visibility,
                    ) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVisibility)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn KeyDown<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::Input::KeyRoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        let handler = <super::Input::KeyEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveKeyDown,
                            ))
                        }
                    }
                    fn ISliderFactory<R, F: FnOnce(&ISliderFactory) -> windows_core::Result<R>>(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<Slider, ISliderFactory> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    fn ISliderStatics<R, F: FnOnce(&ISliderStatics) -> windows_core::Result<R>>(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<Slider, ISliderStatics> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for Slider {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, ISlider>();
                }
                unsafe impl windows_core::Interface for Slider {
                    type Vtable = <ISlider as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID = <ISlider as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for Slider {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Slider";
                }
                unsafe impl Send for Slider {}
                unsafe impl Sync for Slider {}
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct StackPanel(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    StackPanel,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    StackPanel,
                    Panel,
                    super::FrameworkElement,
                    super::UIElement,
                    super::DependencyObject
                );
                impl StackPanel {
                    pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn Width(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Width)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Height(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Height)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn HorizontalAlignment(
                        &self,
                    ) -> windows_core::Result<super::HorizontalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHorizontalAlignment(
                        &self,
                        value: super::HorizontalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn VerticalAlignment(
                        &self,
                    ) -> windows_core::Result<super::VerticalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).VerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVerticalAlignment(
                        &self,
                        value: super::VerticalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Margin(&self) -> windows_core::Result<super::Thickness> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Margin)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMargin(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMargin)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Name)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetName(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetName)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn Children(&self) -> windows_core::Result<UIElementCollection> {
                        let this = &windows_core::Interface::cast::<IPanel>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Children)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn Orientation(&self) -> windows_core::Result<Orientation> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Orientation)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetOrientation(&self, value: Orientation) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetOrientation)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Padding(&self) -> windows_core::Result<super::Thickness> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Padding)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetPadding(&self, value: super::Thickness) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetPadding)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Spacing(&self) -> windows_core::Result<f64> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Spacing)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetSpacing(&self, value: f64) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetSpacing)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn new() -> windows_core::Result<Self> {
                        Self::IStackPanelFactory(|this| unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).CreateInstance)(
                                windows_core::Interface::as_raw(this),
                                core::ptr::null_mut(),
                                core::ptr::null_mut(),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        })
                    }
                    pub fn compose<T>(compose: T) -> windows_core::Result<Self>
                    where
                        T: windows_core::Compose,
                    {
                        Self::IStackPanelFactory(|this| unsafe {
                            let (derived__, base__) = windows_core::Compose::compose(compose);
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).CreateInstance)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(&derived__),
                                base__ as *mut _ as _,
                                &mut result__,
                            )
                            .ok()?;
                            let _ = &derived__;
                            windows_core::imp::Type::from_abi(result__)
                        })
                    }
                    pub fn Opacity(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Opacity)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetOpacity)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Visibility(&self) -> windows_core::Result<super::Visibility> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Visibility)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVisibility(
                        &self,
                        value: super::Visibility,
                    ) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVisibility)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn KeyDown<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::Input::KeyRoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        let handler = <super::Input::KeyEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveKeyDown,
                            ))
                        }
                    }
                    fn IStackPanelFactory<
                        R,
                        F: FnOnce(&IStackPanelFactory) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            StackPanel,
                            IStackPanelFactory,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    fn IStackPanelStatics<
                        R,
                        F: FnOnce(&IStackPanelStatics) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            StackPanel,
                            IStackPanelStatics,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for StackPanel {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, IStackPanel>();
                }
                unsafe impl windows_core::Interface for StackPanel {
                    type Vtable = <IStackPanel as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID = <IStackPanel as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for StackPanel {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.StackPanel";
                }
                unsafe impl Send for StackPanel {}
                unsafe impl Sync for StackPanel {}
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct TextBlock(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    TextBlock,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    TextBlock,
                    super::FrameworkElement,
                    super::UIElement,
                    super::DependencyObject
                );
                impl TextBlock {
                    pub fn new() -> windows_core::Result<Self> {
                        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
                    }
                    fn IActivationFactory<
                        R,
                        F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            TextBlock,
                            windows_core::imp::IGenericFactory,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn Width(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Width)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Height(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Height)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn HorizontalAlignment(
                        &self,
                    ) -> windows_core::Result<super::HorizontalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHorizontalAlignment(
                        &self,
                        value: super::HorizontalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn VerticalAlignment(
                        &self,
                    ) -> windows_core::Result<super::VerticalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).VerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVerticalAlignment(
                        &self,
                        value: super::VerticalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Margin(&self) -> windows_core::Result<super::Thickness> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Margin)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMargin(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMargin)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Name)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetName(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetName)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn FontSize(&self) -> windows_core::Result<f64> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).FontSize)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetFontSize)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn TextWrapping(&self) -> windows_core::Result<super::TextWrapping> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).TextWrapping)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetTextWrapping(
                        &self,
                        value: super::TextWrapping,
                    ) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetTextWrapping)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Text(&self) -> windows_core::Result<windows_core::HSTRING> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Text)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetText(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetText)(
                                windows_core::Interface::as_raw(self),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn IsTextSelectionEnabled(&self) -> windows_core::Result<bool> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).IsTextSelectionEnabled)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsTextSelectionEnabled(
                        &self,
                        value: bool,
                    ) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetIsTextSelectionEnabled)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MaxLines(&self) -> windows_core::Result<i32> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).MaxLines)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMaxLines(&self, value: i32) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetMaxLines)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Opacity(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Opacity)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetOpacity)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Visibility(&self) -> windows_core::Result<super::Visibility> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Visibility)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVisibility(
                        &self,
                        value: super::Visibility,
                    ) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVisibility)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn KeyDown<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::Input::KeyRoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        let handler = <super::Input::KeyEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveKeyDown,
                            ))
                        }
                    }
                    fn ITextBlockStatics<
                        R,
                        F: FnOnce(&ITextBlockStatics) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            TextBlock,
                            ITextBlockStatics,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for TextBlock {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, ITextBlock>();
                }
                unsafe impl windows_core::Interface for TextBlock {
                    type Vtable = <ITextBlock as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID = <ITextBlock as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for TextBlock {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.TextBlock";
                }
                unsafe impl Send for TextBlock {}
                unsafe impl Sync for TextBlock {}
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct TextBox(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    TextBox,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    TextBox,
                    Control,
                    super::FrameworkElement,
                    super::UIElement,
                    super::DependencyObject
                );
                impl TextBox {
                    pub fn FontSize(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).FontSize)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetFontSize)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsEnabled(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsEnabled)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsEnabled(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsEnabled)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Padding(&self) -> windows_core::Result<super::Thickness> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Padding)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetPadding(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<IControl>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetPadding)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).ActualHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn Width(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Width)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Height(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Height)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinWidth(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinWidth)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinWidth)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn MinHeight(&self) -> windows_core::Result<f64> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MinHeight)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMinHeight)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn HorizontalAlignment(
                        &self,
                    ) -> windows_core::Result<super::HorizontalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHorizontalAlignment(
                        &self,
                        value: super::HorizontalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn VerticalAlignment(
                        &self,
                    ) -> windows_core::Result<super::VerticalAlignment> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).VerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVerticalAlignment(
                        &self,
                        value: super::VerticalAlignment,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Margin(&self) -> windows_core::Result<super::Thickness> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Margin)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMargin(&self, value: super::Thickness) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetMargin)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Name)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetName(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        let this =
                            &windows_core::Interface::cast::<super::IFrameworkElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetName)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn Text(&self) -> windows_core::Result<windows_core::HSTRING> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Text)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetText(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetText)(
                                windows_core::Interface::as_raw(self),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn MaxLength(&self) -> windows_core::Result<i32> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).MaxLength)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetMaxLength(&self, value: i32) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetMaxLength)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsReadOnly(&self) -> windows_core::Result<bool> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).IsReadOnly)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsReadOnly(&self, value: bool) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetIsReadOnly)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn AcceptsReturn(&self) -> windows_core::Result<bool> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).AcceptsReturn)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetAcceptsReturn(&self, value: bool) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetAcceptsReturn)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn TextWrapping(&self) -> windows_core::Result<super::TextWrapping> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).TextWrapping)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetTextWrapping(
                        &self,
                        value: super::TextWrapping,
                    ) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetTextWrapping)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Header(&self) -> windows_core::Result<windows_core::IInspectable> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Header)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn SetHeader<P0>(&self, value: P0) -> windows_core::Result<()>
                    where
                        P0: windows_core::Param<windows_core::IInspectable>,
                    {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetHeader)(
                                windows_core::Interface::as_raw(self),
                                value.param().abi(),
                            )
                            .ok()
                        }
                    }
                    pub fn PlaceholderText(&self) -> windows_core::Result<windows_core::HSTRING> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).PlaceholderText)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn SetPlaceholderText(
                        &self,
                        value: &windows_core::HSTRING,
                    ) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetPlaceholderText)(
                                windows_core::Interface::as_raw(self),
                                core::mem::transmute_copy(value),
                            )
                            .ok()
                        }
                    }
                    pub fn TextChanged<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<TextChangedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let handler = <TextChangedEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(self).TextChanged)(
                                windows_core::Interface::as_raw(self),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                self.clone(),
                                token__,
                                windows_core::Interface::vtable(self).RemoveTextChanged,
                            ))
                        }
                    }
                    pub fn BeforeTextChanging<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<Self>,
                                windows_core::Ref<TextBoxBeforeTextChangingEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let handler =
                            <super::super::super::super::Windows::Foundation::TypedEventHandler<
                                Self,
                                TextBoxBeforeTextChangingEventArgs,
                            >>::new(move |a0, a1| {
                                handler(a0, a1);
                                Ok(())
                            });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(self)
                                .BeforeTextChanging)(
                                windows_core::Interface::as_raw(self),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                self.clone(),
                                token__,
                                windows_core::Interface::vtable(self).RemoveBeforeTextChanging,
                            ))
                        }
                    }
                    pub fn Select(&self, start: i32, length: i32) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).Select)(
                                windows_core::Interface::as_raw(self),
                                start,
                                length,
                            )
                            .ok()
                        }
                    }
                    pub fn SelectAll(&self) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SelectAll)(
                                windows_core::Interface::as_raw(self),
                            )
                            .ok()
                        }
                    }
                    pub fn new() -> windows_core::Result<Self> {
                        Self::ITextBoxFactory(|this| unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).CreateInstance)(
                                windows_core::Interface::as_raw(this),
                                core::ptr::null_mut(),
                                core::ptr::null_mut(),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        })
                    }
                    pub fn compose<T>(compose: T) -> windows_core::Result<Self>
                    where
                        T: windows_core::Compose,
                    {
                        Self::ITextBoxFactory(|this| unsafe {
                            let (derived__, base__) = windows_core::Compose::compose(compose);
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).CreateInstance)(
                                windows_core::Interface::as_raw(this),
                                core::mem::transmute_copy(&derived__),
                                base__ as *mut _ as _,
                                &mut result__,
                            )
                            .ok()?;
                            let _ = &derived__;
                            windows_core::imp::Type::from_abi(result__)
                        })
                    }
                    pub fn Opacity(&self) -> windows_core::Result<f64> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Opacity)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetOpacity)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn Visibility(&self) -> windows_core::Result<super::Visibility> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).Visibility)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetVisibility(
                        &self,
                        value: super::Visibility,
                    ) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).SetVisibility)(
                                windows_core::Interface::as_raw(this),
                                value,
                            )
                            .ok()
                        }
                    }
                    pub fn KeyDown<F>(
                        &self,
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<super::Input::KeyRoutedEventArgs>,
                            ) + Send
                            + 'static,
                    {
                        let this = &windows_core::Interface::cast::<super::IUIElement>(self)?;
                        let handler = <super::Input::KeyEventHandler>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveKeyDown,
                            ))
                        }
                    }
                    fn ITextBoxFactory<
                        R,
                        F: FnOnce(&ITextBoxFactory) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<TextBox, ITextBoxFactory> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    fn ITextBoxStatics<
                        R,
                        F: FnOnce(&ITextBoxStatics) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<TextBox, ITextBoxStatics> =
                            windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for TextBox {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, ITextBox>();
                }
                unsafe impl windows_core::Interface for TextBox {
                    type Vtable = <ITextBox as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID = <ITextBox as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for TextBox {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.TextBox";
                }
                unsafe impl Send for TextBox {}
                unsafe impl Sync for TextBox {}
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct TextBoxBeforeTextChangingEventArgs(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    TextBoxBeforeTextChangingEventArgs,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                impl TextBoxBeforeTextChangingEventArgs {
                    pub fn NewText(&self) -> windows_core::Result<windows_core::HSTRING> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).NewText)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| core::mem::transmute(result__))
                        }
                    }
                    pub fn Cancel(&self) -> windows_core::Result<bool> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Cancel)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetCancel(&self, value: bool) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetCancel)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                }
                impl windows_core::RuntimeType for TextBoxBeforeTextChangingEventArgs {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<
                            Self,
                            ITextBoxBeforeTextChangingEventArgs,
                        >();
                }
                unsafe impl windows_core::Interface for TextBoxBeforeTextChangingEventArgs {
                    type Vtable =
                        <ITextBoxBeforeTextChangingEventArgs as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <ITextBoxBeforeTextChangingEventArgs as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for TextBoxBeforeTextChangingEventArgs {
                    const NAME: &'static str =
                        "Microsoft.UI.Xaml.Controls.TextBoxBeforeTextChangingEventArgs";
                }
                unsafe impl Send for TextBoxBeforeTextChangingEventArgs {}
                unsafe impl Sync for TextBoxBeforeTextChangingEventArgs {}
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct TextChangedEventArgs(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    TextChangedEventArgs,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    TextChangedEventArgs,
                    super::RoutedEventArgs
                );
                impl windows_core::RuntimeType for TextChangedEventArgs {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, ITextChangedEventArgs>();
                }
                unsafe impl windows_core::Interface for TextChangedEventArgs {
                    type Vtable = <ITextChangedEventArgs as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <ITextChangedEventArgs as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for TextChangedEventArgs {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.TextChangedEventArgs";
                }
                unsafe impl Send for TextChangedEventArgs {}
                unsafe impl Sync for TextChangedEventArgs {}
                windows_core::imp::define_interface!(
                    TextChangedEventHandler,
                    TextChangedEventHandler_Vtbl,
                    0x5d8ddcff_45d8_5e7c_9b8b_c41d2893c6a1
                );
                impl windows_core::RuntimeType for TextChangedEventHandler {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                }
                impl TextChangedEventHandler {
                    pub fn new<
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<TextChangedEventArgs>,
                            ) -> windows_core::Result<()>
                            + Send
                            + 'static,
                    >(
                        invoke: F,
                    ) -> Self {
                        let com = windows_core::imp::DelegateBox::<Self, F>::new(
                            &TextChangedEventHandlerBox::<F>::VTABLE,
                            invoke,
                        );
                        unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
                    }
                    pub fn Invoke<P0, P1>(&self, sender: P0, e: P1) -> windows_core::Result<()>
                    where
                        P0: windows_core::Param<windows_core::IInspectable>,
                        P1: windows_core::Param<TextChangedEventArgs>,
                    {
                        unsafe {
                            (windows_core::Interface::vtable(self).Invoke)(
                                windows_core::Interface::as_raw(self),
                                sender.param().abi(),
                                e.param().abi(),
                            )
                            .ok()
                        }
                    }
                }
                #[repr(C)]
                pub struct TextChangedEventHandler_Vtbl {
                    base__: windows_core::IUnknown_Vtbl,
                    Invoke: unsafe extern "system" fn(
                        this: *mut core::ffi::c_void,
                        sender: *mut core::ffi::c_void,
                        e: *mut core::ffi::c_void,
                    ) -> windows_core::HRESULT,
                }
                struct TextChangedEventHandlerBox<
                    F: Fn(
                            windows_core::Ref<windows_core::IInspectable>,
                            windows_core::Ref<TextChangedEventArgs>,
                        ) -> windows_core::Result<()>
                        + Send
                        + 'static,
                >(core::marker::PhantomData<(fn() -> F,)>);
                impl<
                    F: Fn(
                            windows_core::Ref<windows_core::IInspectable>,
                            windows_core::Ref<TextChangedEventArgs>,
                        ) -> windows_core::Result<()>
                        + Send
                        + 'static,
                > TextChangedEventHandlerBox<F>
                {
                    const VTABLE: TextChangedEventHandler_Vtbl =
                        TextChangedEventHandler_Vtbl {
                            base__:
                                windows_core::IUnknown_Vtbl {
                                    QueryInterface: windows_core::imp::DelegateBox::<
                                        TextChangedEventHandler,
                                        F,
                                    >::QueryInterface,
                                    AddRef: windows_core::imp::DelegateBox::<
                                        TextChangedEventHandler,
                                        F,
                                    >::AddRef,
                                    Release: windows_core::imp::DelegateBox::<
                                        TextChangedEventHandler,
                                        F,
                                    >::Release,
                                },
                            Invoke: Self::Invoke,
                        };
                    unsafe extern "system" fn Invoke(
                        this: *mut core::ffi::c_void,
                        sender: *mut core::ffi::c_void,
                        e: *mut core::ffi::c_void,
                    ) -> windows_core::HRESULT {
                        unsafe {
                            let this = &mut *(this as *mut *mut core::ffi::c_void
                                as *mut windows_core::imp::DelegateBox<TextChangedEventHandler, F>);
                            (this.invoke)(
                                core::mem::transmute_copy(&sender),
                                core::mem::transmute_copy(&e),
                            )
                            .into()
                        }
                    }
                }
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct UIElementCollection(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    UIElementCollection,
                    windows_core::IUnknown,
                    windows_core::IInspectable,
                    super::super::super::super::Windows::Foundation::Collections::IVector<
                        super::UIElement,
                    >
                );
                impl windows_core::RuntimeType for UIElementCollection {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<
                            Self,
                            super::super::super::super::Windows::Foundation::Collections::IVector<
                                super::UIElement,
                            >,
                        >();
                }
                unsafe impl windows_core::Interface for UIElementCollection {
                    type Vtable =
                        <super::super::super::super::Windows::Foundation::Collections::IVector<
                            super::UIElement,
                        > as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <super::super::super::super::Windows::Foundation::Collections::IVector<
                            super::UIElement,
                        > as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for UIElementCollection {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.UIElementCollection";
                }
                unsafe impl Send for UIElementCollection {}
                unsafe impl Sync for UIElementCollection {}
                impl IntoIterator for UIElementCollection {
                    type Item = super::UIElement;
                    type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                    fn into_iter(self) -> Self::IntoIter {
                        IntoIterator::into_iter(&self)
                    }
                }
                impl IntoIterator for &UIElementCollection {
                    type Item = super::UIElement;
                    type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                    fn into_iter(self) -> Self::IntoIter {
                        windows_collections::BufferedIterator::new(self.First().unwrap())
                    }
                }
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct XamlControlsResources(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    XamlControlsResources,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    XamlControlsResources,
                    super::ResourceDictionary,
                    super::DependencyObject
                );
                impl XamlControlsResources {
                    pub fn new() -> windows_core::Result<Self> {
                        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
                    }
                    fn IActivationFactory<
                        R,
                        F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            XamlControlsResources,
                            windows_core::imp::IGenericFactory,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    pub fn MergedDictionaries(
                        &self,
                    ) -> windows_core::Result<
                        super::super::super::super::Windows::Foundation::Collections::IVector<
                            super::ResourceDictionary,
                        >,
                    > {
                        let this =
                            &windows_core::Interface::cast::<super::IResourceDictionary>(self)?;
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).MergedDictionaries)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    fn IXamlControlsResourcesStatics<
                        R,
                        F: FnOnce(&IXamlControlsResourcesStatics) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            XamlControlsResources,
                            IXamlControlsResourcesStatics,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for XamlControlsResources {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, IXamlControlsResources>();
                }
                unsafe impl windows_core::Interface for XamlControlsResources {
                    type Vtable = <IXamlControlsResources as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <IXamlControlsResources as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for XamlControlsResources {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.XamlControlsResources";
                }
                unsafe impl Send for XamlControlsResources {}
                unsafe impl Sync for XamlControlsResources {}
                impl IntoIterator for XamlControlsResources {
                    type Item = windows_collections::IKeyValuePair<
                        windows_core::IInspectable,
                        windows_core::IInspectable,
                    >;
                    type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                    fn into_iter(self) -> Self::IntoIter {
                        IntoIterator::into_iter(&self)
                    }
                }
                impl IntoIterator for &XamlControlsResources {
                    type Item = windows_collections::IKeyValuePair<
                        windows_core::IInspectable,
                        windows_core::IInspectable,
                    >;
                    type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                    fn into_iter(self) -> Self::IntoIter {
                        windows_collections::BufferedIterator::new(self.First().unwrap())
                    }
                }
                pub mod Primitives {
                    #[repr(transparent)]
                    #[derive(Clone, Debug, Eq, PartialEq)]
                    pub struct ButtonBase(windows_core::IUnknown);
                    windows_core::imp::interface_hierarchy!(
                        ButtonBase,
                        windows_core::IUnknown,
                        windows_core::IInspectable
                    );
                    windows_core::imp::required_hierarchy!(
                        ButtonBase,
                        super::ContentControl,
                        super::Control,
                        super::super::FrameworkElement,
                        super::super::UIElement,
                        super::super::DependencyObject
                    );
                    impl ButtonBase {
                        pub fn Click<F>(
                            &self,
                            handler: F,
                        ) -> windows_core::Result<windows_core::EventRevoker>
                        where
                            F: Fn(
                                    windows_core::Ref<windows_core::IInspectable>,
                                    windows_core::Ref<super::super::RoutedEventArgs>,
                                ) + Send
                                + 'static,
                        {
                            let handler = <super::super::RoutedEventHandler>::new(move |a0, a1| {
                                handler(a0, a1);
                                Ok(())
                            });
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                let token__ = (windows_core::Interface::vtable(self).Click)(
                                    windows_core::Interface::as_raw(self),
                                    windows_core::Interface::as_raw(&handler),
                                    &mut result__,
                                )
                                .map(|| result__)?;
                                Ok(windows_core::EventRevoker::new(
                                    self.clone(),
                                    token__,
                                    windows_core::Interface::vtable(self).RemoveClick,
                                ))
                            }
                        }
                        pub fn Content(&self) -> windows_core::Result<windows_core::IInspectable> {
                            let this =
                                &windows_core::Interface::cast::<super::IContentControl>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Content)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .and_then(|| windows_core::imp::Type::from_abi(result__))
                            }
                        }
                        pub fn SetContent<P0>(&self, value: P0) -> windows_core::Result<()>
                        where
                            P0: windows_core::Param<windows_core::IInspectable>,
                        {
                            let this =
                                &windows_core::Interface::cast::<super::IContentControl>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetContent)(
                                    windows_core::Interface::as_raw(this),
                                    value.param().abi(),
                                )
                                .ok()
                            }
                        }
                        pub fn FontSize(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).FontSize)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetFontSize)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn IsEnabled(&self) -> windows_core::Result<bool> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).IsEnabled)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetIsEnabled(&self, value: bool) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetIsEnabled)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Padding(&self) -> windows_core::Result<super::super::Thickness> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Padding)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetPadding(
                            &self,
                            value: super::super::Thickness,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetPadding)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).ActualWidth)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).ActualHeight)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn Width(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Width)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetWidth)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Height(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Height)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetHeight)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn MinWidth(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).MinWidth)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetMinWidth)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn MinHeight(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).MinHeight)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetMinHeight)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn HorizontalAlignment(
                            &self,
                        ) -> windows_core::Result<super::super::HorizontalAlignment>
                        {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetHorizontalAlignment(
                            &self,
                            value: super::super::HorizontalAlignment,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn VerticalAlignment(
                            &self,
                        ) -> windows_core::Result<super::super::VerticalAlignment>
                        {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).VerticalAlignment)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetVerticalAlignment(
                            &self,
                            value: super::super::VerticalAlignment,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Margin(&self) -> windows_core::Result<super::super::Thickness> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Margin)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetMargin(
                            &self,
                            value: super::super::Thickness,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetMargin)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Name)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| core::mem::transmute(result__))
                            }
                        }
                        pub fn SetName(
                            &self,
                            value: &windows_core::HSTRING,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetName)(
                                    windows_core::Interface::as_raw(this),
                                    core::mem::transmute_copy(value),
                                )
                                .ok()
                            }
                        }
                        pub fn Opacity(&self) -> windows_core::Result<f64> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Opacity)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetOpacity)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Visibility(&self) -> windows_core::Result<super::super::Visibility> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Visibility)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetVisibility(
                            &self,
                            value: super::super::Visibility,
                        ) -> windows_core::Result<()> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetVisibility)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn KeyDown<F>(
                            &self,
                            handler: F,
                        ) -> windows_core::Result<windows_core::EventRevoker>
                        where
                            F: Fn(
                                    windows_core::Ref<windows_core::IInspectable>,
                                    windows_core::Ref<super::super::Input::KeyRoutedEventArgs>,
                                ) + Send
                                + 'static,
                        {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            let handler =
                                <super::super::Input::KeyEventHandler>::new(move |a0, a1| {
                                    handler(a0, a1);
                                    Ok(())
                                });
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                    windows_core::Interface::as_raw(this),
                                    windows_core::Interface::as_raw(&handler),
                                    &mut result__,
                                )
                                .map(|| result__)?;
                                Ok(windows_core::EventRevoker::new(
                                    this.clone(),
                                    token__,
                                    windows_core::Interface::vtable(this).RemoveKeyDown,
                                ))
                            }
                        }
                        fn IButtonBaseFactory<
                            R,
                            F: FnOnce(&IButtonBaseFactory) -> windows_core::Result<R>,
                        >(
                            callback: F,
                        ) -> windows_core::Result<R> {
                            static SHARED: windows_core::imp::FactoryCache<
                                ButtonBase,
                                IButtonBaseFactory,
                            > = windows_core::imp::FactoryCache::new();
                            SHARED.call(callback)
                        }
                        fn IButtonBaseStatics<
                            R,
                            F: FnOnce(&IButtonBaseStatics) -> windows_core::Result<R>,
                        >(
                            callback: F,
                        ) -> windows_core::Result<R> {
                            static SHARED: windows_core::imp::FactoryCache<
                                ButtonBase,
                                IButtonBaseStatics,
                            > = windows_core::imp::FactoryCache::new();
                            SHARED.call(callback)
                        }
                    }
                    impl windows_core::RuntimeType for ButtonBase {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_class::<Self, IButtonBase>();
                    }
                    unsafe impl windows_core::Interface for ButtonBase {
                        type Vtable = <IButtonBase as windows_core::Interface>::Vtable;
                        const IID: windows_core::GUID =
                            <IButtonBase as windows_core::Interface>::IID;
                    }
                    impl windows_core::RuntimeName for ButtonBase {
                        const NAME: &'static str =
                            "Microsoft.UI.Xaml.Controls.Primitives.ButtonBase";
                    }
                    unsafe impl Send for ButtonBase {}
                    unsafe impl Sync for ButtonBase {}
                    windows_core::imp::define_interface!(
                        IButtonBase,
                        IButtonBase_Vtbl,
                        0x65714269_2473_5327_a652_0ea6bce7f403
                    );
                    impl windows_core::RuntimeType for IButtonBase {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Controls.Primitives.IButtonBase",
                            );
                    }
                    #[repr(C)]
                    pub struct IButtonBase_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                        ClickMode: usize,
                        SetClickMode: usize,
                        IsPointerOver: usize,
                        IsPressed: usize,
                        Command: usize,
                        SetCommand: usize,
                        CommandParameter: usize,
                        SetCommandParameter: usize,
                        pub Click: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut core::ffi::c_void,
                            *mut i64,
                        )
                            -> windows_core::HRESULT,
                        pub RemoveClick: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            i64,
                        )
                            -> windows_core::HRESULT,
                    }
                    windows_core::imp::define_interface!(
                        IButtonBaseFactory,
                        IButtonBaseFactory_Vtbl,
                        0x21251aa9_6fd1_5e51_ab3b_e6fcaf3395ed
                    );
                    impl windows_core::RuntimeType for IButtonBaseFactory {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Controls.Primitives.IButtonBaseFactory",
                            );
                    }
                    #[repr(C)]
                    pub struct IButtonBaseFactory_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IButtonBaseStatics,
                        IButtonBaseStatics_Vtbl,
                        0xdbe812f6_adf8_51d3_8137_a8fbf6445b3c
                    );
                    impl windows_core::RuntimeType for IButtonBaseStatics {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Controls.Primitives.IButtonBaseStatics",
                            );
                    }
                    #[repr(C)]
                    pub struct IButtonBaseStatics_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IRangeBase,
                        IRangeBase_Vtbl,
                        0x540d6d61_8fac_5d5c_b5b0_e172a7dde103
                    );
                    impl windows_core::RuntimeType for IRangeBase {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Controls.Primitives.IRangeBase",
                            );
                    }
                    #[repr(C)]
                    pub struct IRangeBase_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                        pub Minimum: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut f64,
                        )
                            -> windows_core::HRESULT,
                        pub SetMinimum: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            f64,
                        )
                            -> windows_core::HRESULT,
                        pub Maximum: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut f64,
                        )
                            -> windows_core::HRESULT,
                        pub SetMaximum: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            f64,
                        )
                            -> windows_core::HRESULT,
                        pub SmallChange: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut f64,
                        )
                            -> windows_core::HRESULT,
                        pub SetSmallChange: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            f64,
                        )
                            -> windows_core::HRESULT,
                        pub LargeChange: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut f64,
                        )
                            -> windows_core::HRESULT,
                        pub SetLargeChange: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            f64,
                        )
                            -> windows_core::HRESULT,
                        pub Value: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut f64,
                        )
                            -> windows_core::HRESULT,
                        pub SetValue: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            f64,
                        )
                            -> windows_core::HRESULT,
                        pub ValueChanged: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut core::ffi::c_void,
                            *mut i64,
                        )
                            -> windows_core::HRESULT,
                        pub RemoveValueChanged: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            i64,
                        )
                            -> windows_core::HRESULT,
                    }
                    windows_core::imp::define_interface!(
                        IRangeBaseFactory,
                        IRangeBaseFactory_Vtbl,
                        0x41c205e2_4422_5dca_9b49_e31210ea396c
                    );
                    impl windows_core::RuntimeType for IRangeBaseFactory {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Controls.Primitives.IRangeBaseFactory",
                            );
                    }
                    #[repr(C)]
                    pub struct IRangeBaseFactory_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IRangeBaseStatics,
                        IRangeBaseStatics_Vtbl,
                        0x4aed5e49_64ec_56f1_874d_b8c0f83f9ac8
                    );
                    impl windows_core::RuntimeType for IRangeBaseStatics {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Controls.Primitives.IRangeBaseStatics",
                            );
                    }
                    #[repr(C)]
                    pub struct IRangeBaseStatics_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IRangeBaseValueChangedEventArgs,
                        IRangeBaseValueChangedEventArgs_Vtbl,
                        0xb0181692_9578_51c7_9d1c_adfcf8945aa9
                    );
                    impl windows_core::RuntimeType for IRangeBaseValueChangedEventArgs {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME : windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice (b"Microsoft.UI.Xaml.Controls.Primitives.IRangeBaseValueChangedEventArgs") ;
                    }
                    #[repr(C)]
                    pub struct IRangeBaseValueChangedEventArgs_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                        pub OldValue: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut f64,
                        )
                            -> windows_core::HRESULT,
                        pub NewValue: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut f64,
                        )
                            -> windows_core::HRESULT,
                    }
                    windows_core::imp::define_interface!(
                        IToggleButton,
                        IToggleButton_Vtbl,
                        0x686fbaa4_c866_568b_8f75_481d8d545291
                    );
                    impl windows_core::RuntimeType for IToggleButton {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Controls.Primitives.IToggleButton",
                            );
                    }
                    #[repr(C)]
                    pub struct IToggleButton_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                        pub IsChecked: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut *mut core::ffi::c_void,
                        )
                            -> windows_core::HRESULT,
                        pub SetIsChecked: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut core::ffi::c_void,
                        )
                            -> windows_core::HRESULT,
                        pub IsThreeState: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut bool,
                        )
                            -> windows_core::HRESULT,
                        pub SetIsThreeState: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            bool,
                        )
                            -> windows_core::HRESULT,
                        pub Checked: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut core::ffi::c_void,
                            *mut i64,
                        )
                            -> windows_core::HRESULT,
                        pub RemoveChecked: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            i64,
                        )
                            -> windows_core::HRESULT,
                        pub Unchecked: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut core::ffi::c_void,
                            *mut i64,
                        )
                            -> windows_core::HRESULT,
                        pub RemoveUnchecked: unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            i64,
                        )
                            -> windows_core::HRESULT,
                    }
                    windows_core::imp::define_interface!(
                        IToggleButtonFactory,
                        IToggleButtonFactory_Vtbl,
                        0x519511bb_d35b_5e2d_966c_8369405a4408
                    );
                    impl windows_core::RuntimeType for IToggleButtonFactory {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Controls.Primitives.IToggleButtonFactory",
                            );
                    }
                    #[repr(C)]
                    pub struct IToggleButtonFactory_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    windows_core::imp::define_interface!(
                        IToggleButtonStatics,
                        IToggleButtonStatics_Vtbl,
                        0x4b8397e3_76fd_59df_824f_40ae339fb00b
                    );
                    impl windows_core::RuntimeType for IToggleButtonStatics {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                        const NAME: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::from_slice(
                                b"Microsoft.UI.Xaml.Controls.Primitives.IToggleButtonStatics",
                            );
                    }
                    #[repr(C)]
                    pub struct IToggleButtonStatics_Vtbl {
                        pub base__: windows_core::IInspectable_Vtbl,
                    }
                    #[repr(transparent)]
                    #[derive(Clone, Debug, Eq, PartialEq)]
                    pub struct RangeBase(windows_core::IUnknown);
                    windows_core::imp::interface_hierarchy!(
                        RangeBase,
                        windows_core::IUnknown,
                        windows_core::IInspectable
                    );
                    windows_core::imp::required_hierarchy!(
                        RangeBase,
                        super::Control,
                        super::super::FrameworkElement,
                        super::super::UIElement,
                        super::super::DependencyObject
                    );
                    impl RangeBase {
                        pub fn FontSize(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).FontSize)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetFontSize)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn IsEnabled(&self) -> windows_core::Result<bool> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).IsEnabled)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetIsEnabled(&self, value: bool) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetIsEnabled)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Padding(&self) -> windows_core::Result<super::super::Thickness> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Padding)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetPadding(
                            &self,
                            value: super::super::Thickness,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetPadding)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).ActualWidth)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).ActualHeight)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn Width(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Width)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetWidth)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Height(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Height)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetHeight)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn MinWidth(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).MinWidth)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetMinWidth)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn MinHeight(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).MinHeight)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetMinHeight)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn HorizontalAlignment(
                            &self,
                        ) -> windows_core::Result<super::super::HorizontalAlignment>
                        {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetHorizontalAlignment(
                            &self,
                            value: super::super::HorizontalAlignment,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn VerticalAlignment(
                            &self,
                        ) -> windows_core::Result<super::super::VerticalAlignment>
                        {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).VerticalAlignment)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetVerticalAlignment(
                            &self,
                            value: super::super::VerticalAlignment,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Margin(&self) -> windows_core::Result<super::super::Thickness> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Margin)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetMargin(
                            &self,
                            value: super::super::Thickness,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetMargin)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Name)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| core::mem::transmute(result__))
                            }
                        }
                        pub fn SetName(
                            &self,
                            value: &windows_core::HSTRING,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetName)(
                                    windows_core::Interface::as_raw(this),
                                    core::mem::transmute_copy(value),
                                )
                                .ok()
                            }
                        }
                        pub fn Minimum(&self) -> windows_core::Result<f64> {
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(self).Minimum)(
                                    windows_core::Interface::as_raw(self),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetMinimum(&self, value: f64) -> windows_core::Result<()> {
                            unsafe {
                                (windows_core::Interface::vtable(self).SetMinimum)(
                                    windows_core::Interface::as_raw(self),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Maximum(&self) -> windows_core::Result<f64> {
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(self).Maximum)(
                                    windows_core::Interface::as_raw(self),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetMaximum(&self, value: f64) -> windows_core::Result<()> {
                            unsafe {
                                (windows_core::Interface::vtable(self).SetMaximum)(
                                    windows_core::Interface::as_raw(self),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn SmallChange(&self) -> windows_core::Result<f64> {
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(self).SmallChange)(
                                    windows_core::Interface::as_raw(self),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetSmallChange(&self, value: f64) -> windows_core::Result<()> {
                            unsafe {
                                (windows_core::Interface::vtable(self).SetSmallChange)(
                                    windows_core::Interface::as_raw(self),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn LargeChange(&self) -> windows_core::Result<f64> {
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(self).LargeChange)(
                                    windows_core::Interface::as_raw(self),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetLargeChange(&self, value: f64) -> windows_core::Result<()> {
                            unsafe {
                                (windows_core::Interface::vtable(self).SetLargeChange)(
                                    windows_core::Interface::as_raw(self),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Value(&self) -> windows_core::Result<f64> {
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(self).Value)(
                                    windows_core::Interface::as_raw(self),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetValue(&self, value: f64) -> windows_core::Result<()> {
                            unsafe {
                                (windows_core::Interface::vtable(self).SetValue)(
                                    windows_core::Interface::as_raw(self),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn ValueChanged<F>(
                            &self,
                            handler: F,
                        ) -> windows_core::Result<windows_core::EventRevoker>
                        where
                            F: Fn(
                                    windows_core::Ref<windows_core::IInspectable>,
                                    windows_core::Ref<RangeBaseValueChangedEventArgs>,
                                ) + Send
                                + 'static,
                        {
                            let handler =
                                <RangeBaseValueChangedEventHandler>::new(move |a0, a1| {
                                    handler(a0, a1);
                                    Ok(())
                                });
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                let token__ = (windows_core::Interface::vtable(self).ValueChanged)(
                                    windows_core::Interface::as_raw(self),
                                    windows_core::Interface::as_raw(&handler),
                                    &mut result__,
                                )
                                .map(|| result__)?;
                                Ok(windows_core::EventRevoker::new(
                                    self.clone(),
                                    token__,
                                    windows_core::Interface::vtable(self).RemoveValueChanged,
                                ))
                            }
                        }
                        pub fn Opacity(&self) -> windows_core::Result<f64> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Opacity)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetOpacity)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Visibility(&self) -> windows_core::Result<super::super::Visibility> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Visibility)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetVisibility(
                            &self,
                            value: super::super::Visibility,
                        ) -> windows_core::Result<()> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetVisibility)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn KeyDown<F>(
                            &self,
                            handler: F,
                        ) -> windows_core::Result<windows_core::EventRevoker>
                        where
                            F: Fn(
                                    windows_core::Ref<windows_core::IInspectable>,
                                    windows_core::Ref<super::super::Input::KeyRoutedEventArgs>,
                                ) + Send
                                + 'static,
                        {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            let handler =
                                <super::super::Input::KeyEventHandler>::new(move |a0, a1| {
                                    handler(a0, a1);
                                    Ok(())
                                });
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                    windows_core::Interface::as_raw(this),
                                    windows_core::Interface::as_raw(&handler),
                                    &mut result__,
                                )
                                .map(|| result__)?;
                                Ok(windows_core::EventRevoker::new(
                                    this.clone(),
                                    token__,
                                    windows_core::Interface::vtable(this).RemoveKeyDown,
                                ))
                            }
                        }
                        fn IRangeBaseFactory<
                            R,
                            F: FnOnce(&IRangeBaseFactory) -> windows_core::Result<R>,
                        >(
                            callback: F,
                        ) -> windows_core::Result<R> {
                            static SHARED: windows_core::imp::FactoryCache<
                                RangeBase,
                                IRangeBaseFactory,
                            > = windows_core::imp::FactoryCache::new();
                            SHARED.call(callback)
                        }
                        fn IRangeBaseStatics<
                            R,
                            F: FnOnce(&IRangeBaseStatics) -> windows_core::Result<R>,
                        >(
                            callback: F,
                        ) -> windows_core::Result<R> {
                            static SHARED: windows_core::imp::FactoryCache<
                                RangeBase,
                                IRangeBaseStatics,
                            > = windows_core::imp::FactoryCache::new();
                            SHARED.call(callback)
                        }
                    }
                    impl windows_core::RuntimeType for RangeBase {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_class::<Self, IRangeBase>();
                    }
                    unsafe impl windows_core::Interface for RangeBase {
                        type Vtable = <IRangeBase as windows_core::Interface>::Vtable;
                        const IID: windows_core::GUID =
                            <IRangeBase as windows_core::Interface>::IID;
                    }
                    impl windows_core::RuntimeName for RangeBase {
                        const NAME: &'static str =
                            "Microsoft.UI.Xaml.Controls.Primitives.RangeBase";
                    }
                    unsafe impl Send for RangeBase {}
                    unsafe impl Sync for RangeBase {}
                    #[repr(transparent)]
                    #[derive(Clone, Debug, Eq, PartialEq)]
                    pub struct RangeBaseValueChangedEventArgs(windows_core::IUnknown);
                    windows_core::imp::interface_hierarchy!(
                        RangeBaseValueChangedEventArgs,
                        windows_core::IUnknown,
                        windows_core::IInspectable
                    );
                    windows_core::imp::required_hierarchy!(
                        RangeBaseValueChangedEventArgs,
                        super::super::RoutedEventArgs
                    );
                    impl RangeBaseValueChangedEventArgs {
                        pub fn OldValue(&self) -> windows_core::Result<f64> {
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(self).OldValue)(
                                    windows_core::Interface::as_raw(self),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn NewValue(&self) -> windows_core::Result<f64> {
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(self).NewValue)(
                                    windows_core::Interface::as_raw(self),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                    }
                    impl windows_core::RuntimeType for RangeBaseValueChangedEventArgs {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_class::<
                                Self,
                                IRangeBaseValueChangedEventArgs,
                            >();
                    }
                    unsafe impl windows_core::Interface for RangeBaseValueChangedEventArgs {
                        type Vtable =
                            <IRangeBaseValueChangedEventArgs as windows_core::Interface>::Vtable;
                        const IID: windows_core::GUID =
                            <IRangeBaseValueChangedEventArgs as windows_core::Interface>::IID;
                    }
                    impl windows_core::RuntimeName for RangeBaseValueChangedEventArgs {
                        const NAME: &'static str =
                            "Microsoft.UI.Xaml.Controls.Primitives.RangeBaseValueChangedEventArgs";
                    }
                    unsafe impl Send for RangeBaseValueChangedEventArgs {}
                    unsafe impl Sync for RangeBaseValueChangedEventArgs {}
                    windows_core::imp::define_interface!(
                        RangeBaseValueChangedEventHandler,
                        RangeBaseValueChangedEventHandler_Vtbl,
                        0x23f0e209_9455_54cb_b8bc_0b49553c7dcc
                    );
                    impl windows_core::RuntimeType for RangeBaseValueChangedEventHandler {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_interface::<Self>();
                    }
                    impl RangeBaseValueChangedEventHandler {
                        pub fn new<
                            F: Fn(
                                    windows_core::Ref<windows_core::IInspectable>,
                                    windows_core::Ref<RangeBaseValueChangedEventArgs>,
                                ) -> windows_core::Result<()>
                                + Send
                                + 'static,
                        >(
                            invoke: F,
                        ) -> Self {
                            let com = windows_core::imp::DelegateBox::<Self, F>::new(
                                &RangeBaseValueChangedEventHandlerBox::<F>::VTABLE,
                                invoke,
                            );
                            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
                        }
                        pub fn Invoke<P0, P1>(&self, sender: P0, e: P1) -> windows_core::Result<()>
                        where
                            P0: windows_core::Param<windows_core::IInspectable>,
                            P1: windows_core::Param<RangeBaseValueChangedEventArgs>,
                        {
                            unsafe {
                                (windows_core::Interface::vtable(self).Invoke)(
                                    windows_core::Interface::as_raw(self),
                                    sender.param().abi(),
                                    e.param().abi(),
                                )
                                .ok()
                            }
                        }
                    }
                    #[repr(C)]
                    pub struct RangeBaseValueChangedEventHandler_Vtbl {
                        base__: windows_core::IUnknown_Vtbl,
                        Invoke: unsafe extern "system" fn(
                            this: *mut core::ffi::c_void,
                            sender: *mut core::ffi::c_void,
                            e: *mut core::ffi::c_void,
                        )
                            -> windows_core::HRESULT,
                    }
                    struct RangeBaseValueChangedEventHandlerBox<
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<RangeBaseValueChangedEventArgs>,
                            ) -> windows_core::Result<()>
                            + Send
                            + 'static,
                    >(core::marker::PhantomData<(fn() -> F,)>);
                    impl<
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<RangeBaseValueChangedEventArgs>,
                            ) -> windows_core::Result<()>
                            + Send
                            + 'static,
                    > RangeBaseValueChangedEventHandlerBox<F>
                    {
                        const VTABLE: RangeBaseValueChangedEventHandler_Vtbl =
                            RangeBaseValueChangedEventHandler_Vtbl {
                                base__: windows_core::IUnknown_Vtbl {
                                    QueryInterface: windows_core::imp::DelegateBox::<
                                        RangeBaseValueChangedEventHandler,
                                        F,
                                    >::QueryInterface,
                                    AddRef: windows_core::imp::DelegateBox::<
                                        RangeBaseValueChangedEventHandler,
                                        F,
                                    >::AddRef,
                                    Release: windows_core::imp::DelegateBox::<
                                        RangeBaseValueChangedEventHandler,
                                        F,
                                    >::Release,
                                },
                                Invoke: Self::Invoke,
                            };
                        unsafe extern "system" fn Invoke(
                            this: *mut core::ffi::c_void,
                            sender: *mut core::ffi::c_void,
                            e: *mut core::ffi::c_void,
                        ) -> windows_core::HRESULT {
                            unsafe {
                                let this = &mut *(this as *mut *mut core::ffi::c_void
                                    as *mut windows_core::imp::DelegateBox<
                                        RangeBaseValueChangedEventHandler,
                                        F,
                                    >);
                                (this.invoke)(
                                    core::mem::transmute_copy(&sender),
                                    core::mem::transmute_copy(&e),
                                )
                                .into()
                            }
                        }
                    }
                    #[repr(transparent)]
                    #[derive(Clone, Debug, Eq, PartialEq)]
                    pub struct ToggleButton(windows_core::IUnknown);
                    windows_core::imp::interface_hierarchy!(
                        ToggleButton,
                        windows_core::IUnknown,
                        windows_core::IInspectable
                    );
                    windows_core::imp::required_hierarchy!(
                        ToggleButton,
                        ButtonBase,
                        super::ContentControl,
                        super::Control,
                        super::super::FrameworkElement,
                        super::super::UIElement,
                        super::super::DependencyObject
                    );
                    impl ToggleButton {
                        pub fn Click<F>(
                            &self,
                            handler: F,
                        ) -> windows_core::Result<windows_core::EventRevoker>
                        where
                            F: Fn(
                                    windows_core::Ref<windows_core::IInspectable>,
                                    windows_core::Ref<super::super::RoutedEventArgs>,
                                ) + Send
                                + 'static,
                        {
                            let this = &windows_core::Interface::cast::<IButtonBase>(self)?;
                            let handler = <super::super::RoutedEventHandler>::new(move |a0, a1| {
                                handler(a0, a1);
                                Ok(())
                            });
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                let token__ = (windows_core::Interface::vtable(this).Click)(
                                    windows_core::Interface::as_raw(this),
                                    windows_core::Interface::as_raw(&handler),
                                    &mut result__,
                                )
                                .map(|| result__)?;
                                Ok(windows_core::EventRevoker::new(
                                    this.clone(),
                                    token__,
                                    windows_core::Interface::vtable(this).RemoveClick,
                                ))
                            }
                        }
                        pub fn Content(&self) -> windows_core::Result<windows_core::IInspectable> {
                            let this =
                                &windows_core::Interface::cast::<super::IContentControl>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Content)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .and_then(|| windows_core::imp::Type::from_abi(result__))
                            }
                        }
                        pub fn SetContent<P0>(&self, value: P0) -> windows_core::Result<()>
                        where
                            P0: windows_core::Param<windows_core::IInspectable>,
                        {
                            let this =
                                &windows_core::Interface::cast::<super::IContentControl>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetContent)(
                                    windows_core::Interface::as_raw(this),
                                    value.param().abi(),
                                )
                                .ok()
                            }
                        }
                        pub fn FontSize(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).FontSize)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetFontSize)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn IsEnabled(&self) -> windows_core::Result<bool> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).IsEnabled)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetIsEnabled(&self, value: bool) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetIsEnabled)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Padding(&self) -> windows_core::Result<super::super::Thickness> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Padding)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetPadding(
                            &self,
                            value: super::super::Thickness,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<super::IControl>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetPadding)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn ActualWidth(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).ActualWidth)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn ActualHeight(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).ActualHeight)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn Width(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Width)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetWidth)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Height(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Height)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetHeight)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn MinWidth(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).MinWidth)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetMinWidth)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn MinHeight(&self) -> windows_core::Result<f64> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).MinHeight)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetMinHeight(&self, value: f64) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetMinHeight)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn HorizontalAlignment(
                            &self,
                        ) -> windows_core::Result<super::super::HorizontalAlignment>
                        {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).HorizontalAlignment)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetHorizontalAlignment(
                            &self,
                            value: super::super::HorizontalAlignment,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetHorizontalAlignment)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn VerticalAlignment(
                            &self,
                        ) -> windows_core::Result<super::super::VerticalAlignment>
                        {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).VerticalAlignment)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetVerticalAlignment(
                            &self,
                            value: super::super::VerticalAlignment,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetVerticalAlignment)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Margin(&self) -> windows_core::Result<super::super::Thickness> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Margin)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetMargin(
                            &self,
                            value: super::super::Thickness,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetMargin)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Name)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| core::mem::transmute(result__))
                            }
                        }
                        pub fn SetName(
                            &self,
                            value: &windows_core::HSTRING,
                        ) -> windows_core::Result<()> {
                            let this = &windows_core::Interface::cast::<
                                super::super::IFrameworkElement,
                            >(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetName)(
                                    windows_core::Interface::as_raw(this),
                                    core::mem::transmute_copy(value),
                                )
                                .ok()
                            }
                        }
                        pub fn IsChecked(&self) -> windows_core::Result<bool> {
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(self).IsChecked)(
                                    windows_core::Interface::as_raw(self),
                                    &mut result__,
                                )
                                .and_then(|| windows_core::imp::Type::from_abi(result__))
                                .and_then(|r__: windows_reference::IReference<bool>| r__.Value())
                            }
                        }
                        pub fn SetIsChecked(
                            &self,
                            value: Option<bool>,
                        ) -> windows_core::Result<()> {
                            let value__ =
                                value.map(<windows_reference::IReference<bool> as From<_>>::from);
                            unsafe {
                                (windows_core::Interface::vtable(self).SetIsChecked)(
                                    windows_core::Interface::as_raw(self),
                                    windows_core::Param::param(value__.as_ref()).abi(),
                                )
                                .ok()
                            }
                        }
                        pub fn IsThreeState(&self) -> windows_core::Result<bool> {
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(self).IsThreeState)(
                                    windows_core::Interface::as_raw(self),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetIsThreeState(&self, value: bool) -> windows_core::Result<()> {
                            unsafe {
                                (windows_core::Interface::vtable(self).SetIsThreeState)(
                                    windows_core::Interface::as_raw(self),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Checked<F>(
                            &self,
                            handler: F,
                        ) -> windows_core::Result<windows_core::EventRevoker>
                        where
                            F: Fn(
                                    windows_core::Ref<windows_core::IInspectable>,
                                    windows_core::Ref<super::super::RoutedEventArgs>,
                                ) + Send
                                + 'static,
                        {
                            let handler = <super::super::RoutedEventHandler>::new(move |a0, a1| {
                                handler(a0, a1);
                                Ok(())
                            });
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                let token__ = (windows_core::Interface::vtable(self).Checked)(
                                    windows_core::Interface::as_raw(self),
                                    windows_core::Interface::as_raw(&handler),
                                    &mut result__,
                                )
                                .map(|| result__)?;
                                Ok(windows_core::EventRevoker::new(
                                    self.clone(),
                                    token__,
                                    windows_core::Interface::vtable(self).RemoveChecked,
                                ))
                            }
                        }
                        pub fn Unchecked<F>(
                            &self,
                            handler: F,
                        ) -> windows_core::Result<windows_core::EventRevoker>
                        where
                            F: Fn(
                                    windows_core::Ref<windows_core::IInspectable>,
                                    windows_core::Ref<super::super::RoutedEventArgs>,
                                ) + Send
                                + 'static,
                        {
                            let handler = <super::super::RoutedEventHandler>::new(move |a0, a1| {
                                handler(a0, a1);
                                Ok(())
                            });
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                let token__ = (windows_core::Interface::vtable(self).Unchecked)(
                                    windows_core::Interface::as_raw(self),
                                    windows_core::Interface::as_raw(&handler),
                                    &mut result__,
                                )
                                .map(|| result__)?;
                                Ok(windows_core::EventRevoker::new(
                                    self.clone(),
                                    token__,
                                    windows_core::Interface::vtable(self).RemoveUnchecked,
                                ))
                            }
                        }
                        pub fn Opacity(&self) -> windows_core::Result<f64> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Opacity)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetOpacity(&self, value: f64) -> windows_core::Result<()> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetOpacity)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).IsHitTestVisible)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetIsHitTestVisible)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn Visibility(&self) -> windows_core::Result<super::super::Visibility> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                (windows_core::Interface::vtable(this).Visibility)(
                                    windows_core::Interface::as_raw(this),
                                    &mut result__,
                                )
                                .map(|| result__)
                            }
                        }
                        pub fn SetVisibility(
                            &self,
                            value: super::super::Visibility,
                        ) -> windows_core::Result<()> {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            unsafe {
                                (windows_core::Interface::vtable(this).SetVisibility)(
                                    windows_core::Interface::as_raw(this),
                                    value,
                                )
                                .ok()
                            }
                        }
                        pub fn KeyDown<F>(
                            &self,
                            handler: F,
                        ) -> windows_core::Result<windows_core::EventRevoker>
                        where
                            F: Fn(
                                    windows_core::Ref<windows_core::IInspectable>,
                                    windows_core::Ref<super::super::Input::KeyRoutedEventArgs>,
                                ) + Send
                                + 'static,
                        {
                            let this =
                                &windows_core::Interface::cast::<super::super::IUIElement>(self)?;
                            let handler =
                                <super::super::Input::KeyEventHandler>::new(move |a0, a1| {
                                    handler(a0, a1);
                                    Ok(())
                                });
                            unsafe {
                                let mut result__ = core::mem::zeroed();
                                let token__ = (windows_core::Interface::vtable(this).KeyDown)(
                                    windows_core::Interface::as_raw(this),
                                    windows_core::Interface::as_raw(&handler),
                                    &mut result__,
                                )
                                .map(|| result__)?;
                                Ok(windows_core::EventRevoker::new(
                                    this.clone(),
                                    token__,
                                    windows_core::Interface::vtable(this).RemoveKeyDown,
                                ))
                            }
                        }
                        fn IToggleButtonFactory<
                            R,
                            F: FnOnce(&IToggleButtonFactory) -> windows_core::Result<R>,
                        >(
                            callback: F,
                        ) -> windows_core::Result<R> {
                            static SHARED: windows_core::imp::FactoryCache<
                                ToggleButton,
                                IToggleButtonFactory,
                            > = windows_core::imp::FactoryCache::new();
                            SHARED.call(callback)
                        }
                        fn IToggleButtonStatics<
                            R,
                            F: FnOnce(&IToggleButtonStatics) -> windows_core::Result<R>,
                        >(
                            callback: F,
                        ) -> windows_core::Result<R> {
                            static SHARED: windows_core::imp::FactoryCache<
                                ToggleButton,
                                IToggleButtonStatics,
                            > = windows_core::imp::FactoryCache::new();
                            SHARED.call(callback)
                        }
                    }
                    impl windows_core::RuntimeType for ToggleButton {
                        const SIGNATURE: windows_core::imp::ConstBuffer =
                            windows_core::imp::ConstBuffer::for_class::<Self, IToggleButton>();
                    }
                    unsafe impl windows_core::Interface for ToggleButton {
                        type Vtable = <IToggleButton as windows_core::Interface>::Vtable;
                        const IID: windows_core::GUID =
                            <IToggleButton as windows_core::Interface>::IID;
                    }
                    impl windows_core::RuntimeName for ToggleButton {
                        const NAME: &'static str =
                            "Microsoft.UI.Xaml.Controls.Primitives.ToggleButton";
                    }
                    unsafe impl Send for ToggleButton {}
                    unsafe impl Sync for ToggleButton {}
                }
            }
            pub mod Hosting {
                windows_core::imp::define_interface!(
                    IWindowsXamlManager,
                    IWindowsXamlManager_Vtbl,
                    0x85a2e562_7e8f_5333_a104_a3e672a2ffee
                );
                impl windows_core::RuntimeType for IWindowsXamlManager {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Hosting.IWindowsXamlManager",
                        );
                }
                #[repr(C)]
                pub struct IWindowsXamlManager_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    IWindowsXamlManagerStatics,
                    IWindowsXamlManagerStatics_Vtbl,
                    0x56cb591d_de97_539f_881d_8ccdc44fa6c4
                );
                impl windows_core::RuntimeType for IWindowsXamlManagerStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Hosting.IWindowsXamlManagerStatics",
                        );
                }
                #[repr(C)]
                pub struct IWindowsXamlManagerStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub InitializeForCurrentThread:
                        unsafe extern "system" fn(
                            *mut core::ffi::c_void,
                            *mut *mut core::ffi::c_void,
                        ) -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IWindowsXamlManagerStatics2,
                    IWindowsXamlManagerStatics2_Vtbl,
                    0x1062430e_0898_5240_ba52_89d9225e7e58
                );
                impl windows_core::RuntimeType for IWindowsXamlManagerStatics2 {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Hosting.IWindowsXamlManagerStatics2",
                        );
                }
                #[repr(C)]
                pub struct IWindowsXamlManagerStatics2_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct WindowsXamlManager(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    WindowsXamlManager,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(
                    WindowsXamlManager,
                    super::super::super::super::Windows::Foundation::IClosable
                );
                impl WindowsXamlManager {
                    pub fn Close(&self) -> windows_core::Result<()> {
                        let this = &windows_core::Interface::cast::<
                            super::super::super::super::Windows::Foundation::IClosable,
                        >(self)?;
                        unsafe {
                            (windows_core::Interface::vtable(this).Close)(
                                windows_core::Interface::as_raw(this),
                            )
                            .ok()
                        }
                    }
                    pub fn InitializeForCurrentThread() -> windows_core::Result<Self> {
                        Self::IWindowsXamlManagerStatics(|this| unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(this).InitializeForCurrentThread)(
                                windows_core::Interface::as_raw(this),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        })
                    }
                    fn IWindowsXamlManagerStatics<
                        R,
                        F: FnOnce(&IWindowsXamlManagerStatics) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            WindowsXamlManager,
                            IWindowsXamlManagerStatics,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    fn IWindowsXamlManagerStatics2<
                        R,
                        F: FnOnce(&IWindowsXamlManagerStatics2) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            WindowsXamlManager,
                            IWindowsXamlManagerStatics2,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for WindowsXamlManager {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, IWindowsXamlManager>();
                }
                unsafe impl windows_core::Interface for WindowsXamlManager {
                    type Vtable = <IWindowsXamlManager as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <IWindowsXamlManager as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for WindowsXamlManager {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Hosting.WindowsXamlManager";
                }
                unsafe impl Send for WindowsXamlManager {}
                unsafe impl Sync for WindowsXamlManager {}
            }
            pub mod Input {
                windows_core::imp::define_interface!(
                    IKeyRoutedEventArgs,
                    IKeyRoutedEventArgs_Vtbl,
                    0xee357007_a2d6_5c75_9431_05fd66ec7915
                );
                impl windows_core::RuntimeType for IKeyRoutedEventArgs {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Input.IKeyRoutedEventArgs",
                        );
                }
                #[repr(C)]
                pub struct IKeyRoutedEventArgs_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub Key: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut super::super::super::super::Windows::System::VirtualKey,
                    )
                        -> windows_core::HRESULT,
                    KeyStatus: usize,
                    pub Handled: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut bool,
                    )
                        -> windows_core::HRESULT,
                    pub SetHandled: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        bool,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    KeyEventHandler,
                    KeyEventHandler_Vtbl,
                    0xdb68e7cc_9a2b_527d_9989_25284daccc03
                );
                impl windows_core::RuntimeType for KeyEventHandler {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                }
                impl KeyEventHandler {
                    pub fn new<
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<KeyRoutedEventArgs>,
                            ) -> windows_core::Result<()>
                            + Send
                            + 'static,
                    >(
                        invoke: F,
                    ) -> Self {
                        let com = windows_core::imp::DelegateBox::<Self, F>::new(
                            &KeyEventHandlerBox::<F>::VTABLE,
                            invoke,
                        );
                        unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
                    }
                    pub fn Invoke<P0, P1>(&self, sender: P0, e: P1) -> windows_core::Result<()>
                    where
                        P0: windows_core::Param<windows_core::IInspectable>,
                        P1: windows_core::Param<KeyRoutedEventArgs>,
                    {
                        unsafe {
                            (windows_core::Interface::vtable(self).Invoke)(
                                windows_core::Interface::as_raw(self),
                                sender.param().abi(),
                                e.param().abi(),
                            )
                            .ok()
                        }
                    }
                }
                #[repr(C)]
                pub struct KeyEventHandler_Vtbl {
                    base__: windows_core::IUnknown_Vtbl,
                    Invoke: unsafe extern "system" fn(
                        this: *mut core::ffi::c_void,
                        sender: *mut core::ffi::c_void,
                        e: *mut core::ffi::c_void,
                    ) -> windows_core::HRESULT,
                }
                struct KeyEventHandlerBox<
                    F: Fn(
                            windows_core::Ref<windows_core::IInspectable>,
                            windows_core::Ref<KeyRoutedEventArgs>,
                        ) -> windows_core::Result<()>
                        + Send
                        + 'static,
                >(core::marker::PhantomData<(fn() -> F,)>);
                impl<
                    F: Fn(
                            windows_core::Ref<windows_core::IInspectable>,
                            windows_core::Ref<KeyRoutedEventArgs>,
                        ) -> windows_core::Result<()>
                        + Send
                        + 'static,
                > KeyEventHandlerBox<F>
                {
                    const VTABLE: KeyEventHandler_Vtbl = KeyEventHandler_Vtbl {
                        base__: windows_core::IUnknown_Vtbl {
                            QueryInterface:
                                windows_core::imp::DelegateBox::<KeyEventHandler, F>::QueryInterface,
                            AddRef: windows_core::imp::DelegateBox::<KeyEventHandler, F>::AddRef,
                            Release: windows_core::imp::DelegateBox::<KeyEventHandler, F>::Release,
                        },
                        Invoke: Self::Invoke,
                    };
                    unsafe extern "system" fn Invoke(
                        this: *mut core::ffi::c_void,
                        sender: *mut core::ffi::c_void,
                        e: *mut core::ffi::c_void,
                    ) -> windows_core::HRESULT {
                        unsafe {
                            let this = &mut *(this as *mut *mut core::ffi::c_void
                                as *mut windows_core::imp::DelegateBox<KeyEventHandler, F>);
                            (this.invoke)(
                                core::mem::transmute_copy(&sender),
                                core::mem::transmute_copy(&e),
                            )
                            .into()
                        }
                    }
                }
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct KeyRoutedEventArgs(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    KeyRoutedEventArgs,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                windows_core::imp::required_hierarchy!(KeyRoutedEventArgs, super::RoutedEventArgs);
                impl KeyRoutedEventArgs {
                    pub fn Key(
                        &self,
                    ) -> windows_core::Result<super::super::super::super::Windows::System::VirtualKey>
                    {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Key)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn Handled(&self) -> windows_core::Result<bool> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).Handled)(
                                windows_core::Interface::as_raw(self),
                                &mut result__,
                            )
                            .map(|| result__)
                        }
                    }
                    pub fn SetHandled(&self, value: bool) -> windows_core::Result<()> {
                        unsafe {
                            (windows_core::Interface::vtable(self).SetHandled)(
                                windows_core::Interface::as_raw(self),
                                value,
                            )
                            .ok()
                        }
                    }
                }
                impl windows_core::RuntimeType for KeyRoutedEventArgs {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, IKeyRoutedEventArgs>();
                }
                unsafe impl windows_core::Interface for KeyRoutedEventArgs {
                    type Vtable = <IKeyRoutedEventArgs as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <IKeyRoutedEventArgs as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for KeyRoutedEventArgs {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Input.KeyRoutedEventArgs";
                }
                unsafe impl Send for KeyRoutedEventArgs {}
                unsafe impl Sync for KeyRoutedEventArgs {}
            }
            pub mod Markup {
                windows_core::imp::define_interface!(
                    IXamlMetadataProvider,
                    IXamlMetadataProvider_Vtbl,
                    0xa96251f0_2214_5d53_8746_ce99a2593cd7
                );
                impl windows_core::RuntimeType for IXamlMetadataProvider {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Markup.IXamlMetadataProvider",
                        );
                }
                windows_core::imp::interface_hierarchy!(
                    IXamlMetadataProvider,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                impl IXamlMetadataProvider {
                    pub fn GetXamlType(
                        &self,
                        r#type: &super::super::super::super::Windows::UI::Xaml::Interop::TypeName,
                    ) -> windows_core::Result<IXamlType> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).GetXamlType)(
                                windows_core::Interface::as_raw(self),
                                core::mem::transmute_copy(r#type),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn GetXamlTypeByFullName(
                        &self,
                        fullname: &windows_core::HSTRING,
                    ) -> windows_core::Result<IXamlType> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).GetXamlTypeByFullName)(
                                windows_core::Interface::as_raw(self),
                                core::mem::transmute_copy(fullname),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn GetXmlnsDefinitions(
                        &self,
                    ) -> windows_core::Result<windows_core::Array<XmlnsDefinition>>
                    {
                        unsafe {
                            let mut result__ = core::mem::MaybeUninit::zeroed();
                            (windows_core::Interface::vtable(self).GetXmlnsDefinitions)(
                                windows_core::Interface::as_raw(self),
                                windows_core::Array::<XmlnsDefinition>::set_abi_len(
                                    core::mem::transmute(&mut result__),
                                ),
                                result__.as_mut_ptr() as *mut _ as _,
                            )
                            .map(|| result__.assume_init())
                        }
                    }
                }
                impl windows_core::RuntimeName for IXamlMetadataProvider {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Markup.IXamlMetadataProvider";
                }
                pub trait IXamlMetadataProvider_Impl: windows_core::IUnknownImpl {
                    fn GetXamlType(
                        &self,
                        r#type: &super::super::super::super::Windows::UI::Xaml::Interop::TypeName,
                    ) -> windows_core::Result<IXamlType>;
                    fn GetXamlTypeByFullName(
                        &self,
                        fullName: &windows_core::HSTRING,
                    ) -> windows_core::Result<IXamlType>;
                    fn GetXmlnsDefinitions(
                        &self,
                    ) -> windows_core::Result<windows_core::Array<XmlnsDefinition>>;
                }
                impl IXamlMetadataProvider_Vtbl {
                    pub const fn new<Identity: IXamlMetadataProvider_Impl, const OFFSET: isize>()
                    -> Self {
                        unsafe extern "system" fn GetXamlType<
                            Identity: IXamlMetadataProvider_Impl,
                            const OFFSET: isize,
                        >(
                            this: *mut core::ffi::c_void,
                            r#type: core::mem::MaybeUninit<
                                super::super::super::super::Windows::UI::Xaml::Interop::TypeName,
                            >,
                            result__: *mut *mut core::ffi::c_void,
                        ) -> windows_core::HRESULT {
                            unsafe {
                                let this: &Identity = &*((this as *const *const ()).offset(OFFSET)
                                    as *const Identity);
                                match IXamlMetadataProvider_Impl::GetXamlType(
                                    this,
                                    core::mem::transmute(&r#type),
                                ) {
                                    Ok(ok__) => {
                                        result__.write(core::mem::transmute_copy(&ok__));
                                        core::mem::forget(ok__);
                                        windows_core::HRESULT(0)
                                    }
                                    Err(err) => err.into(),
                                }
                            }
                        }
                        unsafe extern "system" fn GetXamlTypeByFullName<
                            Identity: IXamlMetadataProvider_Impl,
                            const OFFSET: isize,
                        >(
                            this: *mut core::ffi::c_void,
                            fullname: *mut core::ffi::c_void,
                            result__: *mut *mut core::ffi::c_void,
                        ) -> windows_core::HRESULT {
                            unsafe {
                                let this: &Identity = &*((this as *const *const ()).offset(OFFSET)
                                    as *const Identity);
                                match IXamlMetadataProvider_Impl::GetXamlTypeByFullName(
                                    this,
                                    core::mem::transmute(&fullname),
                                ) {
                                    Ok(ok__) => {
                                        result__.write(core::mem::transmute_copy(&ok__));
                                        core::mem::forget(ok__);
                                        windows_core::HRESULT(0)
                                    }
                                    Err(err) => err.into(),
                                }
                            }
                        }
                        unsafe extern "system" fn GetXmlnsDefinitions<
                            Identity: IXamlMetadataProvider_Impl,
                            const OFFSET: isize,
                        >(
                            this: *mut core::ffi::c_void,
                            result_size__: *mut u32,
                            result__: *mut *mut core::mem::MaybeUninit<XmlnsDefinition>,
                        ) -> windows_core::HRESULT {
                            unsafe {
                                let this: &Identity = &*((this as *const *const ()).offset(OFFSET)
                                    as *const Identity);
                                match IXamlMetadataProvider_Impl::GetXmlnsDefinitions(this) {
                                    Ok(ok__) => {
                                        let (ok_data__, ok_data_len__) = ok__.into_abi();
                                        result__.write(ok_data__);
                                        result_size__.write(ok_data_len__);
                                        windows_core::HRESULT(0)
                                    }
                                    Err(err) => err.into(),
                                }
                            }
                        }
                        Self {
                            base__: windows_core::IInspectable_Vtbl::new::<
                                Identity,
                                IXamlMetadataProvider,
                                OFFSET,
                            >(),
                            GetXamlType: GetXamlType::<Identity, OFFSET>,
                            GetXamlTypeByFullName: GetXamlTypeByFullName::<Identity, OFFSET>,
                            GetXmlnsDefinitions: GetXmlnsDefinitions::<Identity, OFFSET>,
                        }
                    }
                    pub fn matches(iid: &windows_core::GUID) -> bool {
                        iid == &<IXamlMetadataProvider as windows_core::Interface>::IID
                    }
                }
                #[repr(C)]
                pub struct IXamlMetadataProvider_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub GetXamlType: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        core::mem::MaybeUninit<
                            super::super::super::super::Windows::UI::Xaml::Interop::TypeName,
                        >,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    pub GetXamlTypeByFullName: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut *mut core::ffi::c_void,
                    )
                        -> windows_core::HRESULT,
                    pub GetXmlnsDefinitions: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut u32,
                        *mut *mut core::mem::MaybeUninit<XmlnsDefinition>,
                    )
                        -> windows_core::HRESULT,
                }
                windows_core::imp::define_interface!(
                    IXamlType,
                    IXamlType_Vtbl,
                    0xd24219df_7ec9_57f1_a27b_6af251d9c5bc
                );
                impl windows_core::RuntimeType for IXamlType {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Markup.IXamlType",
                        );
                }
                windows_core::imp::interface_hierarchy!(
                    IXamlType,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                #[repr(C)]
                pub struct IXamlType_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                #[repr(C)]
                #[derive(Clone, Debug, Default, Eq, PartialEq)]
                pub struct XmlnsDefinition {
                    pub XmlNamespace: windows_core::HSTRING,
                    pub Namespace: windows_core::HSTRING,
                }
                impl windows_core::imp::TypeKind for XmlnsDefinition {
                    type TypeKind = windows_core::imp::CloneType;
                }
                impl windows_core::RuntimeType for XmlnsDefinition {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"struct(Microsoft.UI.Xaml.Markup.XmlnsDefinition;string;string)",
                        );
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Markup.XmlnsDefinition",
                        );
                }
            }
            pub mod Media {
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct CompositionTarget(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    CompositionTarget,
                    windows_core::IUnknown,
                    windows_core::IInspectable
                );
                impl CompositionTarget {
                    pub fn Rendering<F>(
                        handler: F,
                    ) -> windows_core::Result<windows_core::EventRevoker>
                    where
                        F: Fn(
                                windows_core::Ref<windows_core::IInspectable>,
                                windows_core::Ref<windows_core::IInspectable>,
                            ) + Send
                            + 'static,
                    {
                        let handler = <super::super::super::super::Windows::Foundation::EventHandler<
                            windows_core::IInspectable,
                        >>::new(move |a0, a1| {
                            handler(a0, a1);
                            Ok(())
                        });
                        Self::ICompositionTargetStatics(|this| unsafe {
                            let mut result__ = core::mem::zeroed();
                            let token__ = (windows_core::Interface::vtable(this).Rendering)(
                                windows_core::Interface::as_raw(this),
                                windows_core::Interface::as_raw(&handler),
                                &mut result__,
                            )
                            .map(|| result__)?;
                            Ok(windows_core::EventRevoker::new(
                                this.clone(),
                                token__,
                                windows_core::Interface::vtable(this).RemoveRendering,
                            ))
                        })
                    }
                    fn ICompositionTargetStatics<
                        R,
                        F: FnOnce(&ICompositionTargetStatics) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            CompositionTarget,
                            ICompositionTargetStatics,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for CompositionTarget {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionTarget>();
                }
                unsafe impl windows_core::Interface for CompositionTarget {
                    type Vtable = <ICompositionTarget as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <ICompositionTarget as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for CompositionTarget {
                    const NAME: &'static str = "Microsoft.UI.Xaml.Media.CompositionTarget";
                }
                unsafe impl Send for CompositionTarget {}
                unsafe impl Sync for CompositionTarget {}
                windows_core::imp::define_interface!(
                    ICompositionTarget,
                    ICompositionTarget_Vtbl,
                    0x7d938324_e3ad_597c_93f6_520725410e68
                );
                impl windows_core::RuntimeType for ICompositionTarget {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Media.ICompositionTarget",
                        );
                }
                #[repr(C)]
                pub struct ICompositionTarget_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                windows_core::imp::define_interface!(
                    ICompositionTargetStatics,
                    ICompositionTargetStatics_Vtbl,
                    0x12a4be6f_6db1_5165_b622_d57ab782745b
                );
                impl windows_core::RuntimeType for ICompositionTargetStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Microsoft.UI.Xaml.Media.ICompositionTargetStatics",
                        );
                }
                #[repr(C)]
                pub struct ICompositionTargetStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                    pub Rendering: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        *mut core::ffi::c_void,
                        *mut i64,
                    )
                        -> windows_core::HRESULT,
                    pub RemoveRendering: unsafe extern "system" fn(
                        *mut core::ffi::c_void,
                        i64,
                    )
                        -> windows_core::HRESULT,
                }
            }
            pub mod XamlTypeInfo {
                windows_core::imp::define_interface!(
                    IXamlControlsXamlMetaDataProviderStatics,
                    IXamlControlsXamlMetaDataProviderStatics_Vtbl,
                    0x2d7eb3fd_ecdb_5084_b7e0_12f9598381ef
                );
                impl windows_core::RuntimeType for IXamlControlsXamlMetaDataProviderStatics {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_interface::<Self>();
                    const NAME : windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice (b"Microsoft.UI.Xaml.XamlTypeInfo.IXamlControlsXamlMetaDataProviderStatics") ;
                }
                #[repr(C)]
                pub struct IXamlControlsXamlMetaDataProviderStatics_Vtbl {
                    pub base__: windows_core::IInspectable_Vtbl,
                }
                #[repr(transparent)]
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub struct XamlControlsXamlMetaDataProvider(windows_core::IUnknown);
                windows_core::imp::interface_hierarchy!(
                    XamlControlsXamlMetaDataProvider,
                    windows_core::IUnknown,
                    windows_core::IInspectable,
                    super::Markup::IXamlMetadataProvider
                );
                impl XamlControlsXamlMetaDataProvider {
                    pub fn new() -> windows_core::Result<Self> {
                        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
                    }
                    fn IActivationFactory<
                        R,
                        F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            XamlControlsXamlMetaDataProvider,
                            windows_core::imp::IGenericFactory,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                    pub fn GetXamlType(
                        &self,
                        r#type: &super::super::super::super::Windows::UI::Xaml::Interop::TypeName,
                    ) -> windows_core::Result<super::Markup::IXamlType> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).GetXamlType)(
                                windows_core::Interface::as_raw(self),
                                core::mem::transmute_copy(r#type),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn GetXamlTypeByFullName(
                        &self,
                        fullname: &windows_core::HSTRING,
                    ) -> windows_core::Result<super::Markup::IXamlType> {
                        unsafe {
                            let mut result__ = core::mem::zeroed();
                            (windows_core::Interface::vtable(self).GetXamlTypeByFullName)(
                                windows_core::Interface::as_raw(self),
                                core::mem::transmute_copy(fullname),
                                &mut result__,
                            )
                            .and_then(|| windows_core::imp::Type::from_abi(result__))
                        }
                    }
                    pub fn GetXmlnsDefinitions(
                        &self,
                    ) -> windows_core::Result<windows_core::Array<super::Markup::XmlnsDefinition>>
                    {
                        unsafe {
                            let mut result__ = core::mem::MaybeUninit::zeroed();
                            (windows_core::Interface::vtable(self).GetXmlnsDefinitions)(
                                windows_core::Interface::as_raw(self),
                                windows_core::Array::<super::Markup::XmlnsDefinition>::set_abi_len(
                                    core::mem::transmute(&mut result__),
                                ),
                                result__.as_mut_ptr() as *mut _ as _,
                            )
                            .map(|| result__.assume_init())
                        }
                    }
                    fn IXamlControlsXamlMetaDataProviderStatics<
                        R,
                        F: FnOnce(
                            &IXamlControlsXamlMetaDataProviderStatics,
                        ) -> windows_core::Result<R>,
                    >(
                        callback: F,
                    ) -> windows_core::Result<R> {
                        static SHARED: windows_core::imp::FactoryCache<
                            XamlControlsXamlMetaDataProvider,
                            IXamlControlsXamlMetaDataProviderStatics,
                        > = windows_core::imp::FactoryCache::new();
                        SHARED.call(callback)
                    }
                }
                impl windows_core::RuntimeType for XamlControlsXamlMetaDataProvider {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::for_class::<
                            Self,
                            super::Markup::IXamlMetadataProvider,
                        >();
                }
                unsafe impl windows_core::Interface for XamlControlsXamlMetaDataProvider {
                    type Vtable =
                        <super::Markup::IXamlMetadataProvider as windows_core::Interface>::Vtable;
                    const IID: windows_core::GUID =
                        <super::Markup::IXamlMetadataProvider as windows_core::Interface>::IID;
                }
                impl windows_core::RuntimeName for XamlControlsXamlMetaDataProvider {
                    const NAME: &'static str =
                        "Microsoft.UI.Xaml.XamlTypeInfo.XamlControlsXamlMetaDataProvider";
                }
                unsafe impl Send for XamlControlsXamlMetaDataProvider {}
                unsafe impl Sync for XamlControlsXamlMetaDataProvider {}
            }
        }
    }
}
pub mod Windows {
    pub mod Foundation {
        #[repr(transparent)]
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct EventHandler<T>(windows_core::IUnknown, core::marker::PhantomData<T>)
        where
            T: windows_core::RuntimeType + 'static;
        unsafe impl<T: windows_core::RuntimeType + 'static> windows_core::Interface for EventHandler<T> {
            type Vtable = EventHandler_Vtbl<T>;
            const IID: windows_core::GUID =
                windows_core::GUID::from_signature(<Self as windows_core::RuntimeType>::SIGNATURE);
        }
        impl<T: windows_core::RuntimeType + 'static> windows_core::RuntimeType for EventHandler<T> {
            const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::new()
                .push_slice(b"pinterface({9de1c535-6ae1-11e0-84e1-18a905bcc53f}")
                .push_slice(b";")
                .push_other(T::SIGNATURE)
                .push_slice(b")");
        }
        impl<T: windows_core::RuntimeType + 'static> EventHandler<T> {
            pub fn new<
                F: Fn(
                        windows_core::Ref<windows_core::IInspectable>,
                        windows_core::Ref<T>,
                    ) -> windows_core::Result<()>
                    + Send
                    + 'static,
            >(
                invoke: F,
            ) -> Self {
                let com = windows_core::imp::DelegateBox::<Self, F>::new(
                    &EventHandlerBox::<T, F>::VTABLE,
                    invoke,
                );
                unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
            }
            pub fn Invoke<P0, P1>(&self, sender: P0, args: P1) -> windows_core::Result<()>
            where
                P0: windows_core::Param<windows_core::IInspectable>,
                P1: windows_core::Param<T>,
            {
                unsafe {
                    (windows_core::Interface::vtable(self).Invoke)(
                        windows_core::Interface::as_raw(self),
                        sender.param().abi(),
                        args.param().abi(),
                    )
                    .ok()
                }
            }
        }
        #[repr(C)]
        pub struct EventHandler_Vtbl<T>
        where
            T: windows_core::RuntimeType + 'static,
        {
            base__: windows_core::IUnknown_Vtbl,
            Invoke: unsafe extern "system" fn(
                this: *mut core::ffi::c_void,
                sender: *mut core::ffi::c_void,
                args: windows_core::imp::AbiType<T>,
            ) -> windows_core::HRESULT,
            T: core::marker::PhantomData<T>,
        }
        struct EventHandlerBox<
            T,
            F: Fn(
                    windows_core::Ref<windows_core::IInspectable>,
                    windows_core::Ref<T>,
                ) -> windows_core::Result<()>
                + Send
                + 'static,
        >(core::marker::PhantomData<(T, fn() -> F)>)
        where
            T: windows_core::RuntimeType + 'static;
        impl<
            T: windows_core::RuntimeType + 'static,
            F: Fn(
                    windows_core::Ref<windows_core::IInspectable>,
                    windows_core::Ref<T>,
                ) -> windows_core::Result<()>
                + Send
                + 'static,
        > EventHandlerBox<T, F>
        {
            const VTABLE: EventHandler_Vtbl<T> = EventHandler_Vtbl::<T> {
                base__: windows_core::IUnknown_Vtbl {
                    QueryInterface:
                        windows_core::imp::DelegateBox::<EventHandler<T>, F>::QueryInterface,
                    AddRef: windows_core::imp::DelegateBox::<EventHandler<T>, F>::AddRef,
                    Release: windows_core::imp::DelegateBox::<EventHandler<T>, F>::Release,
                },
                Invoke: Self::Invoke,
                T: core::marker::PhantomData::<T>,
            };
            unsafe extern "system" fn Invoke(
                this: *mut core::ffi::c_void,
                sender: *mut core::ffi::c_void,
                args: windows_core::imp::AbiType<T>,
            ) -> windows_core::HRESULT {
                unsafe {
                    let this = &mut *(this as *mut *mut core::ffi::c_void
                        as *mut windows_core::imp::DelegateBox<EventHandler<T>, F>);
                    (this.invoke)(
                        core::mem::transmute_copy(&sender),
                        core::mem::transmute_copy(&args),
                    )
                    .into()
                }
            }
        }
        windows_core::imp::define_interface!(
            IClosable,
            IClosable_Vtbl,
            0x30d5a829_7fa4_4026_83bb_d75bae4ea99e
        );
        impl windows_core::RuntimeType for IClosable {
            const SIGNATURE: windows_core::imp::ConstBuffer =
                windows_core::imp::ConstBuffer::for_interface::<Self>();
            const NAME: windows_core::imp::ConstBuffer =
                windows_core::imp::ConstBuffer::from_slice(b"Windows.Foundation.IClosable");
        }
        windows_core::imp::interface_hierarchy!(
            IClosable,
            windows_core::IUnknown,
            windows_core::IInspectable
        );
        impl IClosable {
            pub fn Close(&self) -> windows_core::Result<()> {
                unsafe {
                    (windows_core::Interface::vtable(self).Close)(windows_core::Interface::as_raw(
                        self,
                    ))
                    .ok()
                }
            }
        }
        #[repr(C)]
        pub struct IClosable_Vtbl {
            pub base__: windows_core::IInspectable_Vtbl,
            pub Close: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
        }
        windows_core::imp::define_interface!(
            IPropertyValue,
            IPropertyValue_Vtbl,
            0x4bd682dd_7554_40e9_9a9b_82654ede7e62
        );
        impl windows_core::RuntimeType for IPropertyValue {
            const SIGNATURE: windows_core::imp::ConstBuffer =
                windows_core::imp::ConstBuffer::for_interface::<Self>();
            const NAME: windows_core::imp::ConstBuffer =
                windows_core::imp::ConstBuffer::from_slice(b"Windows.Foundation.IPropertyValue");
        }
        windows_core::imp::interface_hierarchy!(
            IPropertyValue,
            windows_core::IUnknown,
            windows_core::IInspectable
        );
        impl IPropertyValue {
            pub fn Type(&self) -> windows_core::Result<PropertyType> {
                unsafe {
                    let mut result__ = core::mem::zeroed();
                    (windows_core::Interface::vtable(self).Type)(
                        windows_core::Interface::as_raw(self),
                        &mut result__,
                    )
                    .map(|| result__)
                }
            }
            pub fn GetInt32(&self) -> windows_core::Result<i32> {
                unsafe {
                    let mut result__ = core::mem::zeroed();
                    (windows_core::Interface::vtable(self).GetInt32)(
                        windows_core::Interface::as_raw(self),
                        &mut result__,
                    )
                    .map(|| result__)
                }
            }
            pub fn GetDouble(&self) -> windows_core::Result<f64> {
                unsafe {
                    let mut result__ = core::mem::zeroed();
                    (windows_core::Interface::vtable(self).GetDouble)(
                        windows_core::Interface::as_raw(self),
                        &mut result__,
                    )
                    .map(|| result__)
                }
            }
            pub fn GetBoolean(&self) -> windows_core::Result<bool> {
                unsafe {
                    let mut result__ = core::mem::zeroed();
                    (windows_core::Interface::vtable(self).GetBoolean)(
                        windows_core::Interface::as_raw(self),
                        &mut result__,
                    )
                    .map(|| result__)
                }
            }
            pub fn GetString(&self) -> windows_core::Result<windows_core::HSTRING> {
                unsafe {
                    let mut result__ = core::mem::zeroed();
                    (windows_core::Interface::vtable(self).GetString)(
                        windows_core::Interface::as_raw(self),
                        &mut result__,
                    )
                    .map(|| core::mem::transmute(result__))
                }
            }
        }
        #[repr(C)]
        pub struct IPropertyValue_Vtbl {
            pub base__: windows_core::IInspectable_Vtbl,
            pub Type: unsafe extern "system" fn(
                *mut core::ffi::c_void,
                *mut PropertyType,
            ) -> windows_core::HRESULT,
            IsNumericScalar: usize,
            GetUInt8: usize,
            GetInt16: usize,
            GetUInt16: usize,
            pub GetInt32: unsafe extern "system" fn(
                *mut core::ffi::c_void,
                *mut i32,
            ) -> windows_core::HRESULT,
            GetUInt32: usize,
            GetInt64: usize,
            GetUInt64: usize,
            GetSingle: usize,
            pub GetDouble: unsafe extern "system" fn(
                *mut core::ffi::c_void,
                *mut f64,
            ) -> windows_core::HRESULT,
            GetChar16: usize,
            pub GetBoolean: unsafe extern "system" fn(
                *mut core::ffi::c_void,
                *mut bool,
            ) -> windows_core::HRESULT,
            pub GetString: unsafe extern "system" fn(
                *mut core::ffi::c_void,
                *mut *mut core::ffi::c_void,
            ) -> windows_core::HRESULT,
        }
        windows_core::imp::define_interface!(
            IPropertyValueStatics,
            IPropertyValueStatics_Vtbl,
            0x629bdbc8_d932_4ff4_96b9_8d96c5c1e858
        );
        impl windows_core::RuntimeType for IPropertyValueStatics {
            const SIGNATURE: windows_core::imp::ConstBuffer =
                windows_core::imp::ConstBuffer::for_interface::<Self>();
            const NAME: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
                b"Windows.Foundation.IPropertyValueStatics",
            );
        }
        #[repr(C)]
        pub struct IPropertyValueStatics_Vtbl {
            pub base__: windows_core::IInspectable_Vtbl,
            CreateEmpty: usize,
            CreateUInt8: usize,
            CreateInt16: usize,
            CreateUInt16: usize,
            pub CreateInt32: unsafe extern "system" fn(
                *mut core::ffi::c_void,
                i32,
                *mut *mut core::ffi::c_void,
            ) -> windows_core::HRESULT,
            CreateUInt32: usize,
            CreateInt64: usize,
            CreateUInt64: usize,
            CreateSingle: usize,
            pub CreateDouble: unsafe extern "system" fn(
                *mut core::ffi::c_void,
                f64,
                *mut *mut core::ffi::c_void,
            ) -> windows_core::HRESULT,
            CreateChar16: usize,
            pub CreateBoolean: unsafe extern "system" fn(
                *mut core::ffi::c_void,
                bool,
                *mut *mut core::ffi::c_void,
            ) -> windows_core::HRESULT,
            pub CreateString: unsafe extern "system" fn(
                *mut core::ffi::c_void,
                *mut core::ffi::c_void,
                *mut *mut core::ffi::c_void,
            ) -> windows_core::HRESULT,
        }
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct PropertyType(pub i32);
        impl PropertyType {
            pub const Empty: Self = Self(0);
            pub const UInt8: Self = Self(1);
            pub const Int16: Self = Self(2);
            pub const UInt16: Self = Self(3);
            pub const Int32: Self = Self(4);
            pub const UInt32: Self = Self(5);
            pub const Int64: Self = Self(6);
            pub const UInt64: Self = Self(7);
            pub const Single: Self = Self(8);
            pub const Double: Self = Self(9);
            pub const Char16: Self = Self(10);
            pub const Boolean: Self = Self(11);
            pub const String: Self = Self(12);
            pub const Inspectable: Self = Self(13);
            pub const DateTime: Self = Self(14);
            pub const TimeSpan: Self = Self(15);
            pub const Guid: Self = Self(16);
            pub const Point: Self = Self(17);
            pub const Size: Self = Self(18);
            pub const Rect: Self = Self(19);
            pub const OtherType: Self = Self(20);
            pub const UInt8Array: Self = Self(1025);
            pub const Int16Array: Self = Self(1026);
            pub const UInt16Array: Self = Self(1027);
            pub const Int32Array: Self = Self(1028);
            pub const UInt32Array: Self = Self(1029);
            pub const Int64Array: Self = Self(1030);
            pub const UInt64Array: Self = Self(1031);
            pub const SingleArray: Self = Self(1032);
            pub const DoubleArray: Self = Self(1033);
            pub const Char16Array: Self = Self(1034);
            pub const BooleanArray: Self = Self(1035);
            pub const StringArray: Self = Self(1036);
            pub const InspectableArray: Self = Self(1037);
            pub const DateTimeArray: Self = Self(1038);
            pub const TimeSpanArray: Self = Self(1039);
            pub const GuidArray: Self = Self(1040);
            pub const PointArray: Self = Self(1041);
            pub const SizeArray: Self = Self(1042);
            pub const RectArray: Self = Self(1043);
            pub const OtherTypeArray: Self = Self(1044);
        }
        impl windows_core::imp::TypeKind for PropertyType {
            type TypeKind = windows_core::imp::CopyType;
        }
        impl windows_core::RuntimeType for PropertyType {
            const SIGNATURE: windows_core::imp::ConstBuffer =
                windows_core::imp::ConstBuffer::from_slice(
                    b"enum(Windows.Foundation.PropertyType;i4)",
                );
            const NAME: windows_core::imp::ConstBuffer =
                windows_core::imp::ConstBuffer::from_slice(b"Windows.Foundation.PropertyType");
        }
        pub struct PropertyValue;
        impl PropertyValue {
            pub fn CreateInt32(value: i32) -> windows_core::Result<windows_core::IInspectable> {
                Self::IPropertyValueStatics(|this| unsafe {
                    let mut result__ = core::mem::zeroed();
                    (windows_core::Interface::vtable(this).CreateInt32)(
                        windows_core::Interface::as_raw(this),
                        value,
                        &mut result__,
                    )
                    .and_then(|| windows_core::imp::Type::from_abi(result__))
                })
            }
            pub fn CreateDouble(value: f64) -> windows_core::Result<windows_core::IInspectable> {
                Self::IPropertyValueStatics(|this| unsafe {
                    let mut result__ = core::mem::zeroed();
                    (windows_core::Interface::vtable(this).CreateDouble)(
                        windows_core::Interface::as_raw(this),
                        value,
                        &mut result__,
                    )
                    .and_then(|| windows_core::imp::Type::from_abi(result__))
                })
            }
            pub fn CreateBoolean(value: bool) -> windows_core::Result<windows_core::IInspectable> {
                Self::IPropertyValueStatics(|this| unsafe {
                    let mut result__ = core::mem::zeroed();
                    (windows_core::Interface::vtable(this).CreateBoolean)(
                        windows_core::Interface::as_raw(this),
                        value,
                        &mut result__,
                    )
                    .and_then(|| windows_core::imp::Type::from_abi(result__))
                })
            }
            pub fn CreateString(
                value: &windows_core::HSTRING,
            ) -> windows_core::Result<windows_core::IInspectable> {
                Self::IPropertyValueStatics(|this| unsafe {
                    let mut result__ = core::mem::zeroed();
                    (windows_core::Interface::vtable(this).CreateString)(
                        windows_core::Interface::as_raw(this),
                        core::mem::transmute_copy(value),
                        &mut result__,
                    )
                    .and_then(|| windows_core::imp::Type::from_abi(result__))
                })
            }
            fn IPropertyValueStatics<
                R,
                F: FnOnce(&IPropertyValueStatics) -> windows_core::Result<R>,
            >(
                callback: F,
            ) -> windows_core::Result<R> {
                static SHARED: windows_core::imp::FactoryCache<
                    PropertyValue,
                    IPropertyValueStatics,
                > = windows_core::imp::FactoryCache::new();
                SHARED.call(callback)
            }
        }
        impl windows_core::RuntimeName for PropertyValue {
            const NAME: &'static str = "Windows.Foundation.PropertyValue";
        }
        #[repr(transparent)]
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct TypedEventHandler<TSender, TResult>(
            windows_core::IUnknown,
            core::marker::PhantomData<TSender>,
            core::marker::PhantomData<TResult>,
        )
        where
            TSender: windows_core::RuntimeType + 'static,
            TResult: windows_core::RuntimeType + 'static;
        unsafe impl<
            TSender: windows_core::RuntimeType + 'static,
            TResult: windows_core::RuntimeType + 'static,
        > windows_core::Interface for TypedEventHandler<TSender, TResult>
        {
            type Vtable = TypedEventHandler_Vtbl<TSender, TResult>;
            const IID: windows_core::GUID =
                windows_core::GUID::from_signature(<Self as windows_core::RuntimeType>::SIGNATURE);
        }
        impl<
            TSender: windows_core::RuntimeType + 'static,
            TResult: windows_core::RuntimeType + 'static,
        > windows_core::RuntimeType for TypedEventHandler<TSender, TResult>
        {
            const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::new()
                .push_slice(b"pinterface({9de1c534-6ae1-11e0-84e1-18a905bcc53f}")
                .push_slice(b";")
                .push_other(TSender::SIGNATURE)
                .push_slice(b";")
                .push_other(TResult::SIGNATURE)
                .push_slice(b")");
        }
        impl<
            TSender: windows_core::RuntimeType + 'static,
            TResult: windows_core::RuntimeType + 'static,
        > TypedEventHandler<TSender, TResult>
        {
            pub fn new<
                F: Fn(
                        windows_core::Ref<TSender>,
                        windows_core::Ref<TResult>,
                    ) -> windows_core::Result<()>
                    + Send
                    + 'static,
            >(
                invoke: F,
            ) -> Self {
                let com = windows_core::imp::DelegateBox::<Self, F>::new(
                    &TypedEventHandlerBox::<TSender, TResult, F>::VTABLE,
                    invoke,
                );
                unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
            }
            pub fn Invoke<P0, P1>(&self, sender: P0, args: P1) -> windows_core::Result<()>
            where
                P0: windows_core::Param<TSender>,
                P1: windows_core::Param<TResult>,
            {
                unsafe {
                    (windows_core::Interface::vtable(self).Invoke)(
                        windows_core::Interface::as_raw(self),
                        sender.param().abi(),
                        args.param().abi(),
                    )
                    .ok()
                }
            }
        }
        #[repr(C)]
        pub struct TypedEventHandler_Vtbl<TSender, TResult>
        where
            TSender: windows_core::RuntimeType + 'static,
            TResult: windows_core::RuntimeType + 'static,
        {
            base__: windows_core::IUnknown_Vtbl,
            Invoke: unsafe extern "system" fn(
                this: *mut core::ffi::c_void,
                sender: windows_core::imp::AbiType<TSender>,
                args: windows_core::imp::AbiType<TResult>,
            ) -> windows_core::HRESULT,
            TSender: core::marker::PhantomData<TSender>,
            TResult: core::marker::PhantomData<TResult>,
        }
        struct TypedEventHandlerBox<
            TSender,
            TResult,
            F: Fn(
                    windows_core::Ref<TSender>,
                    windows_core::Ref<TResult>,
                ) -> windows_core::Result<()>
                + Send
                + 'static,
        >(core::marker::PhantomData<(TSender, TResult, fn() -> F)>)
        where
            TSender: windows_core::RuntimeType + 'static,
            TResult: windows_core::RuntimeType + 'static;
        impl<
            TSender: windows_core::RuntimeType + 'static,
            TResult: windows_core::RuntimeType + 'static,
            F: Fn(
                    windows_core::Ref<TSender>,
                    windows_core::Ref<TResult>,
                ) -> windows_core::Result<()>
                + Send
                + 'static,
        > TypedEventHandlerBox<TSender, TResult, F>
        {
            const VTABLE: TypedEventHandler_Vtbl<TSender, TResult> =
                TypedEventHandler_Vtbl::<TSender, TResult> {
                    base__: windows_core::IUnknown_Vtbl {
                        QueryInterface: windows_core::imp::DelegateBox::<
                            TypedEventHandler<TSender, TResult>,
                            F,
                        >::QueryInterface,
                        AddRef: windows_core::imp::DelegateBox::<
                            TypedEventHandler<TSender, TResult>,
                            F,
                        >::AddRef,
                        Release: windows_core::imp::DelegateBox::<
                            TypedEventHandler<TSender, TResult>,
                            F,
                        >::Release,
                    },
                    Invoke: Self::Invoke,
                    TSender: core::marker::PhantomData::<TSender>,
                    TResult: core::marker::PhantomData::<TResult>,
                };
            unsafe extern "system" fn Invoke(
                this: *mut core::ffi::c_void,
                sender: windows_core::imp::AbiType<TSender>,
                args: windows_core::imp::AbiType<TResult>,
            ) -> windows_core::HRESULT {
                unsafe {
                    let this = &mut *(this as *mut *mut core::ffi::c_void
                        as *mut windows_core::imp::DelegateBox<
                            TypedEventHandler<TSender, TResult>,
                            F,
                        >);
                    (this.invoke)(
                        core::mem::transmute_copy(&sender),
                        core::mem::transmute_copy(&args),
                    )
                    .into()
                }
            }
        }
        pub mod Collections {
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct IIterable<T>(windows_core::IUnknown, core::marker::PhantomData<T>)
            where
                T: windows_core::RuntimeType + 'static;
            impl<T: windows_core::RuntimeType + 'static>
                windows_core::imp::CanInto<windows_core::IUnknown> for IIterable<T>
            {
            }
            impl<T: windows_core::RuntimeType + 'static>
                windows_core::imp::CanInto<windows_core::IInspectable> for IIterable<T>
            {
            }
            unsafe impl<T: windows_core::RuntimeType + 'static> windows_core::Interface for IIterable<T> {
                type Vtable = IIterable_Vtbl<T>;
                const IID: windows_core::GUID = windows_core::GUID::from_signature(
                    <Self as windows_core::RuntimeType>::SIGNATURE,
                );
            }
            impl<T: windows_core::RuntimeType + 'static> windows_core::RuntimeType for IIterable<T> {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::new()
                        .push_slice(b"pinterface({faa585ea-6214-4217-afda-7f46de5869b3}")
                        .push_slice(b";")
                        .push_other(T::SIGNATURE)
                        .push_slice(b")");
                const NAME: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::new()
                    .push_slice(b"Windows.Foundation.Collections.IIterable`1<")
                    .push_other(T::NAME)
                    .push_slice(b">");
            }
            #[repr(C)]
            pub struct IIterable_Vtbl<T>
            where
                T: windows_core::RuntimeType + 'static,
            {
                pub base__: windows_core::IInspectable_Vtbl,
                T: core::marker::PhantomData<T>,
            }
            impl<T: windows_core::RuntimeType> IntoIterator for IIterable<T> {
                type Item = T;
                type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                fn into_iter(self) -> Self::IntoIter {
                    IntoIterator::into_iter(&self)
                }
            }
            impl<T: windows_core::RuntimeType> IntoIterator for &IIterable<T> {
                type Item = T;
                type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                fn into_iter(self) -> Self::IntoIter {
                    windows_collections::BufferedIterator::new(self.First().unwrap())
                }
            }
            #[repr(transparent)]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct IVector<T>(windows_core::IUnknown, core::marker::PhantomData<T>)
            where
                T: windows_core::RuntimeType + 'static;
            impl<T: windows_core::RuntimeType + 'static>
                windows_core::imp::CanInto<windows_core::IUnknown> for IVector<T>
            {
            }
            impl<T: windows_core::RuntimeType + 'static>
                windows_core::imp::CanInto<windows_core::IInspectable> for IVector<T>
            {
            }
            unsafe impl<T: windows_core::RuntimeType + 'static> windows_core::Interface for IVector<T> {
                type Vtable = IVector_Vtbl<T>;
                const IID: windows_core::GUID = windows_core::GUID::from_signature(
                    <Self as windows_core::RuntimeType>::SIGNATURE,
                );
            }
            impl<T: windows_core::RuntimeType + 'static> windows_core::RuntimeType for IVector<T> {
                const SIGNATURE: windows_core::imp::ConstBuffer =
                    windows_core::imp::ConstBuffer::new()
                        .push_slice(b"pinterface({913337e9-11a1-4345-a3a2-4e7f956e222d}")
                        .push_slice(b";")
                        .push_other(T::SIGNATURE)
                        .push_slice(b")");
                const NAME: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::new()
                    .push_slice(b"Windows.Foundation.Collections.IVector`1<")
                    .push_other(T::NAME)
                    .push_slice(b">");
            }
            impl<T: windows_core::RuntimeType + 'static> IntoIterator for IVector<T> {
                type Item = T;
                type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                fn into_iter(self) -> Self::IntoIter {
                    IntoIterator::into_iter(&self)
                }
            }
            impl<T: windows_core::RuntimeType + 'static> IntoIterator for &IVector<T> {
                type Item = T;
                type IntoIter = windows_collections::BufferedIterator<Self::Item>;
                fn into_iter(self) -> Self::IntoIter {
                    windows_collections::BufferedIterator::new(self.First().unwrap())
                }
            }
            #[repr(C)]
            pub struct IVector_Vtbl<T>
            where
                T: windows_core::RuntimeType + 'static,
            {
                pub base__: windows_core::IInspectable_Vtbl,
                T: core::marker::PhantomData<T>,
            }
        }
    }
    pub mod System {
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct VirtualKey(pub i32);
        impl VirtualKey {
            pub const None: Self = Self(0);
            pub const LeftButton: Self = Self(1);
            pub const RightButton: Self = Self(2);
            pub const Cancel: Self = Self(3);
            pub const MiddleButton: Self = Self(4);
            pub const XButton1: Self = Self(5);
            pub const XButton2: Self = Self(6);
            pub const Back: Self = Self(8);
            pub const Tab: Self = Self(9);
            pub const Clear: Self = Self(12);
            pub const Enter: Self = Self(13);
            pub const Shift: Self = Self(16);
            pub const Control: Self = Self(17);
            pub const Menu: Self = Self(18);
            pub const Pause: Self = Self(19);
            pub const CapitalLock: Self = Self(20);
            pub const Kana: Self = Self(21);
            pub const Hangul: Self = Self(21);
            pub const ImeOn: Self = Self(22);
            pub const Junja: Self = Self(23);
            pub const Final: Self = Self(24);
            pub const Hanja: Self = Self(25);
            pub const Kanji: Self = Self(25);
            pub const ImeOff: Self = Self(26);
            pub const Escape: Self = Self(27);
            pub const Convert: Self = Self(28);
            pub const NonConvert: Self = Self(29);
            pub const Accept: Self = Self(30);
            pub const ModeChange: Self = Self(31);
            pub const Space: Self = Self(32);
            pub const PageUp: Self = Self(33);
            pub const PageDown: Self = Self(34);
            pub const End: Self = Self(35);
            pub const Home: Self = Self(36);
            pub const Left: Self = Self(37);
            pub const Up: Self = Self(38);
            pub const Right: Self = Self(39);
            pub const Down: Self = Self(40);
            pub const Select: Self = Self(41);
            pub const Print: Self = Self(42);
            pub const Execute: Self = Self(43);
            pub const Snapshot: Self = Self(44);
            pub const Insert: Self = Self(45);
            pub const Delete: Self = Self(46);
            pub const Help: Self = Self(47);
            pub const Number0: Self = Self(48);
            pub const Number1: Self = Self(49);
            pub const Number2: Self = Self(50);
            pub const Number3: Self = Self(51);
            pub const Number4: Self = Self(52);
            pub const Number5: Self = Self(53);
            pub const Number6: Self = Self(54);
            pub const Number7: Self = Self(55);
            pub const Number8: Self = Self(56);
            pub const Number9: Self = Self(57);
            pub const A: Self = Self(65);
            pub const B: Self = Self(66);
            pub const C: Self = Self(67);
            pub const D: Self = Self(68);
            pub const E: Self = Self(69);
            pub const F: Self = Self(70);
            pub const G: Self = Self(71);
            pub const H: Self = Self(72);
            pub const I: Self = Self(73);
            pub const J: Self = Self(74);
            pub const K: Self = Self(75);
            pub const L: Self = Self(76);
            pub const M: Self = Self(77);
            pub const N: Self = Self(78);
            pub const O: Self = Self(79);
            pub const P: Self = Self(80);
            pub const Q: Self = Self(81);
            pub const R: Self = Self(82);
            pub const S: Self = Self(83);
            pub const T: Self = Self(84);
            pub const U: Self = Self(85);
            pub const V: Self = Self(86);
            pub const W: Self = Self(87);
            pub const X: Self = Self(88);
            pub const Y: Self = Self(89);
            pub const Z: Self = Self(90);
            pub const LeftWindows: Self = Self(91);
            pub const RightWindows: Self = Self(92);
            pub const Application: Self = Self(93);
            pub const Sleep: Self = Self(95);
            pub const NumberPad0: Self = Self(96);
            pub const NumberPad1: Self = Self(97);
            pub const NumberPad2: Self = Self(98);
            pub const NumberPad3: Self = Self(99);
            pub const NumberPad4: Self = Self(100);
            pub const NumberPad5: Self = Self(101);
            pub const NumberPad6: Self = Self(102);
            pub const NumberPad7: Self = Self(103);
            pub const NumberPad8: Self = Self(104);
            pub const NumberPad9: Self = Self(105);
            pub const Multiply: Self = Self(106);
            pub const Add: Self = Self(107);
            pub const Separator: Self = Self(108);
            pub const Subtract: Self = Self(109);
            pub const Decimal: Self = Self(110);
            pub const Divide: Self = Self(111);
            pub const F1: Self = Self(112);
            pub const F2: Self = Self(113);
            pub const F3: Self = Self(114);
            pub const F4: Self = Self(115);
            pub const F5: Self = Self(116);
            pub const F6: Self = Self(117);
            pub const F7: Self = Self(118);
            pub const F8: Self = Self(119);
            pub const F9: Self = Self(120);
            pub const F10: Self = Self(121);
            pub const F11: Self = Self(122);
            pub const F12: Self = Self(123);
            pub const F13: Self = Self(124);
            pub const F14: Self = Self(125);
            pub const F15: Self = Self(126);
            pub const F16: Self = Self(127);
            pub const F17: Self = Self(128);
            pub const F18: Self = Self(129);
            pub const F19: Self = Self(130);
            pub const F20: Self = Self(131);
            pub const F21: Self = Self(132);
            pub const F22: Self = Self(133);
            pub const F23: Self = Self(134);
            pub const F24: Self = Self(135);
            pub const NavigationView: Self = Self(136);
            pub const NavigationMenu: Self = Self(137);
            pub const NavigationUp: Self = Self(138);
            pub const NavigationDown: Self = Self(139);
            pub const NavigationLeft: Self = Self(140);
            pub const NavigationRight: Self = Self(141);
            pub const NavigationAccept: Self = Self(142);
            pub const NavigationCancel: Self = Self(143);
            pub const NumberKeyLock: Self = Self(144);
            pub const Scroll: Self = Self(145);
            pub const LeftShift: Self = Self(160);
            pub const RightShift: Self = Self(161);
            pub const LeftControl: Self = Self(162);
            pub const RightControl: Self = Self(163);
            pub const LeftMenu: Self = Self(164);
            pub const RightMenu: Self = Self(165);
            pub const GoBack: Self = Self(166);
            pub const GoForward: Self = Self(167);
            pub const Refresh: Self = Self(168);
            pub const Stop: Self = Self(169);
            pub const Search: Self = Self(170);
            pub const Favorites: Self = Self(171);
            pub const GoHome: Self = Self(172);
            pub const GamepadA: Self = Self(195);
            pub const GamepadB: Self = Self(196);
            pub const GamepadX: Self = Self(197);
            pub const GamepadY: Self = Self(198);
            pub const GamepadRightShoulder: Self = Self(199);
            pub const GamepadLeftShoulder: Self = Self(200);
            pub const GamepadLeftTrigger: Self = Self(201);
            pub const GamepadRightTrigger: Self = Self(202);
            pub const GamepadDPadUp: Self = Self(203);
            pub const GamepadDPadDown: Self = Self(204);
            pub const GamepadDPadLeft: Self = Self(205);
            pub const GamepadDPadRight: Self = Self(206);
            pub const GamepadMenu: Self = Self(207);
            pub const GamepadView: Self = Self(208);
            pub const GamepadLeftThumbstickButton: Self = Self(209);
            pub const GamepadRightThumbstickButton: Self = Self(210);
            pub const GamepadLeftThumbstickUp: Self = Self(211);
            pub const GamepadLeftThumbstickDown: Self = Self(212);
            pub const GamepadLeftThumbstickRight: Self = Self(213);
            pub const GamepadLeftThumbstickLeft: Self = Self(214);
            pub const GamepadRightThumbstickUp: Self = Self(215);
            pub const GamepadRightThumbstickDown: Self = Self(216);
            pub const GamepadRightThumbstickRight: Self = Self(217);
            pub const GamepadRightThumbstickLeft: Self = Self(218);
        }
        impl windows_core::imp::TypeKind for VirtualKey {
            type TypeKind = windows_core::imp::CopyType;
        }
        impl windows_core::RuntimeType for VirtualKey {
            const SIGNATURE: windows_core::imp::ConstBuffer =
                windows_core::imp::ConstBuffer::from_slice(b"enum(Windows.System.VirtualKey;i4)");
            const NAME: windows_core::imp::ConstBuffer =
                windows_core::imp::ConstBuffer::from_slice(b"Windows.System.VirtualKey");
        }
    }
    pub mod UI {
        pub mod Xaml {
            pub mod Interop {
                #[repr(transparent)]
                #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
                pub struct TypeKind(pub i32);
                impl TypeKind {
                    pub const Primitive: Self = Self(0);
                    pub const Metadata: Self = Self(1);
                    pub const Custom: Self = Self(2);
                }
                impl windows_core::imp::TypeKind for TypeKind {
                    type TypeKind = windows_core::imp::CopyType;
                }
                impl windows_core::RuntimeType for TypeKind {
                    const SIGNATURE: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"enum(Windows.UI.Xaml.Interop.TypeKind;i4)",
                        );
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Windows.UI.Xaml.Interop.TypeKind",
                        );
                }
                #[repr(C)]
                #[derive(Clone, Debug, Default, Eq, PartialEq)]
                pub struct TypeName {
                    pub Name: windows_core::HSTRING,
                    pub Kind: TypeKind,
                }
                impl windows_core::imp::TypeKind for TypeName {
                    type TypeKind = windows_core::imp::CloneType;
                }
                impl windows_core::RuntimeType for TypeName {
                    const SIGNATURE : windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice (b"struct(Windows.UI.Xaml.Interop.TypeName;string;enum(Windows.UI.Xaml.Interop.TypeKind;i4))") ;
                    const NAME: windows_core::imp::ConstBuffer =
                        windows_core::imp::ConstBuffer::from_slice(
                            b"Windows.UI.Xaml.Interop.TypeName",
                        );
                }
            }
        }
    }
    pub mod Win32 {
        #[inline]
        pub unsafe fn DispatchMessageW(lpmsg: *const MSG) -> LRESULT {
            windows_core::link!("user32.dll" "system" fn DispatchMessageW(lpmsg : *const MSG) -> LRESULT);
            unsafe { DispatchMessageW(lpmsg) }
        }
        #[inline]
        pub unsafe fn GetMessageW(
            lpmsg: *mut MSG,
            hwnd: Option<HWND>,
            wmsgfiltermin: u32,
            wmsgfiltermax: u32,
        ) -> windows_core::BOOL {
            windows_core::link!("user32.dll" "system" fn GetMessageW(lpmsg : *mut MSG, hwnd : HWND, wmsgfiltermin : u32, wmsgfiltermax : u32) -> windows_core::BOOL);
            unsafe {
                GetMessageW(
                    lpmsg as _,
                    hwnd.unwrap_or(core::mem::zeroed()) as _,
                    wmsgfiltermin,
                    wmsgfiltermax,
                )
            }
        }
        #[inline]
        pub unsafe fn GetModuleFileNameW(
            hmodule: Option<HMODULE>,
            lpfilename: windows_core::PWSTR,
            nsize: u32,
        ) -> u32 {
            windows_core::link!("kernel32.dll" "system" fn GetModuleFileNameW(hmodule : HMODULE, lpfilename : windows_core::PWSTR, nsize : u32) -> u32);
            unsafe {
                GetModuleFileNameW(
                    hmodule.unwrap_or(core::mem::zeroed()) as _,
                    lpfilename,
                    nsize,
                )
            }
        }
        #[inline]
        pub unsafe fn GetProcAddress<P1>(hmodule: HMODULE, lpprocname: P1) -> FARPROC
        where
            P1: windows_core::Param<windows_core::PCSTR>,
        {
            windows_core::link!("kernel32.dll" "system" fn GetProcAddress(hmodule : HMODULE, lpprocname : windows_core::PCSTR) -> FARPROC);
            unsafe { GetProcAddress(hmodule, lpprocname.param().abi()) }
        }
        #[inline]
        pub unsafe fn LoadLibraryW<P0>(lplibfilename: P0) -> HMODULE
        where
            P0: windows_core::Param<windows_core::PCWSTR>,
        {
            windows_core::link!("kernel32.dll" "system" fn LoadLibraryW(lplibfilename : windows_core::PCWSTR) -> HMODULE);
            unsafe { LoadLibraryW(lplibfilename.param().abi()) }
        }
        #[inline]
        pub unsafe fn PeekMessageW(
            lpmsg: *mut MSG,
            hwnd: Option<HWND>,
            wmsgfiltermin: u32,
            wmsgfiltermax: u32,
            wremovemsg: u32,
        ) -> windows_core::BOOL {
            windows_core::link!("user32.dll" "system" fn PeekMessageW(lpmsg : *mut MSG, hwnd : HWND, wmsgfiltermin : u32, wmsgfiltermax : u32, wremovemsg : u32) -> windows_core::BOOL);
            unsafe {
                PeekMessageW(
                    lpmsg as _,
                    hwnd.unwrap_or(core::mem::zeroed()) as _,
                    wmsgfiltermin,
                    wmsgfiltermax,
                    wremovemsg,
                )
            }
        }
        #[inline]
        pub unsafe fn PostQuitMessage(nexitcode: i32) {
            windows_core::link!("user32.dll" "system" fn PostQuitMessage(nexitcode : i32));
            unsafe { PostQuitMessage(nexitcode) }
        }
        #[inline]
        pub unsafe fn RoInitialize(inittype: RO_INIT_TYPE) -> windows_core::HRESULT {
            windows_core::link!("api-ms-win-core-winrt-l1-1-0.dll" "system" fn RoInitialize(inittype : RO_INIT_TYPE) -> windows_core::HRESULT);
            unsafe { RoInitialize(inittype) }
        }
        #[inline]
        pub unsafe fn TranslateMessage(lpmsg: *const MSG) -> windows_core::BOOL {
            windows_core::link!("user32.dll" "system" fn TranslateMessage(lpmsg : *const MSG) -> windows_core::BOOL);
            unsafe { TranslateMessage(lpmsg) }
        }
        pub type FARPROC = Option<unsafe extern "system" fn() -> isize>;
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
        pub struct HINSTANCE(pub *mut core::ffi::c_void);
        pub type HMODULE = HINSTANCE;
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
        pub struct HWND(pub *mut core::ffi::c_void);
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
        pub struct LPARAM(pub isize);
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
        pub struct LRESULT(pub isize);
        #[repr(C)]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct MSG {
            pub hwnd: HWND,
            pub message: u32,
            pub wParam: WPARAM,
            pub lParam: LPARAM,
            pub time: u32,
            pub pt: POINT,
        }
        pub const PM_REMOVE: i32 = 1;
        #[repr(C)]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct POINT {
            pub x: i32,
            pub y: i32,
        }
        pub const RO_INIT_SINGLETHREADED: RO_INIT_TYPE = 0;
        pub type RO_INIT_TYPE = i32;
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
        pub struct WPARAM(pub usize);
    }
}
