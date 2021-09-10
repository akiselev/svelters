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
    let cls = cls
        .constructor0(|| unsafe { Ok(Qt::new(QWidget::new_0a())) })?
        .superclass::<Qt<QObject>>()?;
    methods!(cls(this: QWidget) {
        show[0]() { this.show() };
        hide[0]() { this.hide() };
        resize[2](w: i32, h: i32) {
            this.resize_2a(w, h)
        };
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
            Q::Box(this.accessible_description());
            this.set_accessible_description(accessibleDescription.to_ref())
        };
        accessibleName: Q<QString> {
            Q::Box(this.accessible_name());
            this.set_accessible_name(accessibleName.to_ref())
        };
        statusTip: Q<QString> {
            Q::Box(this.status_tip());
            this.set_status_tip(statusTip.to_ref())
        };
        styleSheet: Q<QString> {
            Q::Box(this.style_sheet());
            this.set_style_sheet(styleSheet.to_ref())
        };
        toolTip: Q<QString> {
            Q::Box(this.tool_tip());
            this.set_tool_tip(toolTip.to_ref())
        };
        whatsThis: Q<QString> {
            Q::Box(this.whats_this());
            this.set_whats_this(whatsThis.to_ref())
        };
        windowFilePath: Q<QString> {
            Q::Box(this.window_file_path());
            this.set_window_file_path(windowFilePath.to_ref())
        };
        windowTitle: Q<QString> {
            Q::Box(this.window_title());
            this.set_window_title(windowTitle.to_ref())
        };
        /**********************************************
         * ---------------------------
         *  Access functions:
         *  QSize 	baseSize() const
         *  void 	setBaseSize(const QSize &)
         *  void 	setBaseSize(int basew, int baseh)
         *  ---------------------------
         *  Access functions:
         *  QRect 	childrenRect() const
         *  ---------------------------
         *  Access functions:
         *  QRegion 	childrenRegion() const
         *  ---------------------------
         *  Access functions:
         *  Qt::ContextMenuPolicy 	contextMenuPolicy() const
         *  void 	setContextMenuPolicy(Qt::ContextMenuPolicy policy)
         *  ---------------------------
         *  Access functions:
         *  QCursor 	cursor() const
         *  void 	setCursor(const QCursor &)
         *  void 	unsetCursor()
         *  ---------------------------
         *  Access functions:
         *  Qt::FocusPolicy 	focusPolicy() const
         *  void 	setFocusPolicy(Qt::FocusPolicy policy)
         *  ---------------------------
         *  Access functions:
         *  const QFont &	font() const
         *  void 	setFont(const QFont &)
         *  ---------------------------
         *  Access functions:
         *  QRect 	frameGeometry() const
         *  ---------------------------
         *  Access functions:
         *  QSize 	frameSize() const
         *  fullScreen : const bool
         *  ---------------------------
         *  Access functions:
         *  const QRect &	geometry() const
         *  void 	setGeometry(int x, int y, int w, int h)
         *  void 	setGeometry(const QRect &)
         *  ---------------------------
         *  ess functions:
         *  Qt::InputMethodHints 	inputMethodHints() const
         *  void 	setInputMethodHints(Qt::InputMethodHints hints)
         *  ---------------------------
         *  Access functions:
         *  Qt::LayoutDirection 	layoutDirection() const
         *  void 	setLayoutDirection(Qt::LayoutDirection direction)
         *  void 	unsetLayoutDirection()
         *  ---------------------------
         *  Access functions:
         *  QLocale 	locale() const
         *  void 	setLocale(const QLocale &locale)
         *  void 	unsetLocale()
         *  ---------------------------
         *  Access functions:
         *  QSize 	maximumSize() const
         *  void 	setMaximumSize(const QSize &)
         *  void 	setMaximumSize(int maxw, int maxh)
         *  ---------------------------
         *  Access functions:
         *  QSize 	minimumSize() const
         *  void 	setMinimumSize(const QSize &)
         *  void 	setMinimumSize(int minw, int minh)
         *  ---------------------------
         *  Access functions:
         *  virtual QSize 	minimumSizeHint() const
         *  ---------------------------
         *  Access functions:
         *  QRect 	normalGeometry() const
         *  ---------------------------
         *  Access functions:
         *  const QPalette &	palette() const
         *  void 	setPalette(const QPalette &)
         *  ---------------------------
         *  Access functions:
         *  QPoint 	pos() const
         *  void 	move(int x, int y)
         *  void 	move(const QPoint &)
         *  ---------------------------
         *  Access functions:
         *  QRect 	rect() const
         *  ---------------------------
         *  Access functions:
         *  QSize 	size() const
         *  void 	resize(int w, int h)
         *  void 	resize(const QSize &)
         *  ---------------------------
         *  Access functions:
         *  virtual QSize 	sizeHint() const
         *  ---------------------------
         *  Access functions:
         *  QSize 	sizeIncrement() const
         *  void 	setSizeIncrement(const QSize &)
         *  void 	setSizeIncrement(int w, int h)
         *  ---------------------------
         *  Access functions:
         *  QSizePolicy 	sizePolicy() const
         *  void 	setSizePolicy(QSizePolicy)
         *  void 	setSizePolicy(QSizePolicy::Policy horizontal, QSizePolicy::Policy vertical)
         *  ---------------------------
         *  Access functions:
         *  Qt::WindowFlags 	windowFlags() const
         *  void 	setWindowFlags(Qt::WindowFlags type)
         *  ---------------------------
         *  Access functions:
         *  QIcon 	windowIcon() const
         *  void 	setWindowIcon(const QIcon &icon)
         *  ---------------------------
         *  Access functions:
         *  Qt::WindowModality 	windowModality() const
         *  void 	setWindowModality(Qt::WindowModality windowModality)
         *  ---------------------------
         */
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
