use std::{ffi::CStr, ops::Deref, os::raw::c_char};

use frunk::hlist::IntoTuple2;
use qt_core::{
    cpp_core::{CastFrom, CppBox, CppDeletable, NullPtr, Ptr, Ref, StaticDowncast, StaticUpcast},
    QBox, QCoreApplication, QCoreApplicationArgs, QObject, QPtr, QString, SlotOfBool,
};
use qt_gui::QWindow;
use qt_widgets::{QAbstractButton, QApplication, QMainWindow, QPushButton, QWidget};
use rquickjs::{AsFunction, Class, Loaded, Module as QJsModule, Native};

use crate::{
    impls::containers::{DerefWrapper, Qt},
    inner::*,
    QtCore, QtWidgets,
};

define!(|cls: QObject| {
    cls.constructor0(|| unsafe { Ok(Qt::new(QObject::new_0a())) })?
        .method0("className", |this: &Qt<QObject>| -> Result<String> {
            let name = unsafe {
                let obj = this.meta_object();
                CStr::from_ptr(obj.class_name() as *const c_char).to_owned()
            };
            name.into_string().map_err(|err| Error::Exception {
                message: err.to_string(),
                file: file!().to_string(),
                line: line!().try_into().unwrap(),
                stack: "".to_string(),
                rust_stack: Some("place2".to_string()),
            })
        })?
        .method0("test", |this: &Qt<QObject>| {
            println!("REAL >>>>>>>>>>>>>>>>>>");
            Ok(())
        })?
        .static_fn0("staticTest", || {
            println!("STATIC >>>>>>>>>>>>>>>>>>");
            Ok(())
        })?;
});

define!(|cls: QWidget| {
    cls.constructor0(|| unsafe { Ok(Qt::new(QWidget::new_0a())) })?
        .superclass::<Qt<QObject>>()?;
    methods!(cls(this: QWidget) {
        show[0]() { this.show() };
        hide[0]() { this.hide() };
    });
    props!(cls(this: QWidget) {
        height: i32 {
            this.height()
        };
        width: i32 {
            this.width()
        };
        maximumHeight: i32 {
            this.maximum_height();
            this.set_maximum_height(maximumHeight);
        };
        maximumWidth: i32 {
            this.maximum_width();
            this.set_maximum_width(maximumWidth);
        };
        minimumHeight: i32 {
            this.minimum_height();
            this.set_minimum_height(minimumHeight);
        };
        minimumWidth: i32 {
            this.minimum_width();
            this.set_minimum_width(minimumWidth);
        };
        toolTipDuration: i32 {
            this.tool_tip_duration();
            this.set_tool_tip_duration(toolTipDuration);
        };
        windowOpacity: f64 {
            this.window_opacity();
            this.set_window_opacity(windowOpacity);
        };
        acceptDrops: bool {
            this.accept_drops();
            this.set_accept_drops(acceptDrops)
        };
        acceptDrops: bool {
            this.accept_drops();
            this.set_accept_drops(acceptDrops);
        };
        autoFillBackground: bool {
            this.auto_fill_background();
            this.set_auto_fill_background(autoFillBackground);
        };
        enabled: bool {
            this.is_enabled();
            this.set_enabled(enabled);
        };
        hasFocus: bool {
            this.has_focus()
        };
        isFullScreen: bool {
            this.is_full_screen()
        };
        isActiveWindow: bool {
            this.is_active_window()
        };
        isMaximized: bool {
            this.is_maximized()
        };
        isMinimized: bool {
            this.is_minimized()
        };
        isModal: bool {
            this.is_modal()
        };
        hasMouseTracking: bool {
            this.has_mouse_tracking();
            this.set_mouse_tracking(hasMouseTracking);
        };
        hasTabletTracking: bool {
            this.has_tablet_tracking();
            this.set_tablet_tracking(hasTabletTracking);
        };
        updatesEnabled: bool {
            this.updates_enabled();
            this.set_updates_enabled(updatesEnabled);
        };
        visible: bool {
            this.is_visible();
            this.set_visible(visible);
        };
        isWindowModified: bool {
            this.is_window_modified();
            this.set_window_modified(isWindowModified);
        };
        accessibleDescription: Q<QString> {
            Q(this.accessible_description());
            this.set_accessible_description(accessibleDescription.as_ref())
        };
        accessibleName: Q<QString> {
            Q(this.accessible_name());
            this.set_accessible_name(accessibleName.as_ref())
        };
        statusTip: Q<QString> {
            Q(this.status_tip());
            this.set_status_tip(statusTip.as_ref())
        };
        styleSheet: Q<QString> {
            Q(this.style_sheet());
            this.set_style_sheet(styleSheet.as_ref())
        };
        toolTip: Q<QString> {
            Q(this.tool_tip());
            this.set_tool_tip(toolTip.as_ref())
        };
        whatsThis: Q<QString> {
            Q(this.whats_this());
            this.set_whats_this(whatsThis.as_ref())
        };
        windowFilePath: Q<QString> {
            Q(this.window_file_path());
            this.set_window_file_path(windowFilePath.as_ref())
        };
        windowTitle: Q<QString> {
            Q(this.window_title());
            this.set_window_title(windowTitle.as_ref())
        };
    });
});

define!(|cls: QApplication| {
    // cls.static_fn0("exec", || unsafe {
    //     QCoreApplication::exec();
    // })?
    cls.constructor0(|| unsafe {
        let mut args = QCoreApplicationArgs::new();
        let (argc, argv) = args.get();
        std::mem::forget(args);
        Ok(Qt::new(QApplication::new_2a(argc, argv)))
    })?
    .superclass::<Qt<QObject>>()?;
});
