use std::{ffi::CStr, ops::Deref, os::raw::c_char};

use frunk::hlist::IntoTuple2;
use qt_core::{
    cpp_core::{CastFrom, CppBox, CppDeletable, Ptr, Ref, StaticDowncast, StaticUpcast},
    QBox, QCoreApplication, QCoreApplicationArgs, QObject, QPtr, QString,
};
use qt_gui::QWindow;
use qt_widgets::{QApplication, QMainWindow, QPushButton, QWidget};
use rquickjs::{AsFunction, Class, Loaded, Module as QJsModule, Native};

use crate::{
    impls::containers::{DerefWrapper, Qt},
    inner::*,
    QtCore,
};

impl Module for QtCore {
    #[inline(always)]
    fn define<P>(module: &mut ModuleBuilder<Self, P>) -> Result<()>
    where
        QT: Framework,
    {
        module.class::<Qt<QObject>>()?;
        module.class::<Qt<QWidget>>()?;
        module.class::<Qt<QApplication>>()?;
        module.class::<Qt<QMainWindow>>()?;
        module.class::<Qt<QPushButton>>()?;
        Ok(())
    }
}

impl Type for Qt<QObject> {
    const CLASS_NAME: &'static str = "QObject";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        cls.constructor0(|| unsafe { Ok(Qt::new(QObject::new_0a())) })?;
        cls.method0("className", |this: &Qt<QObject>| -> Result<String> {
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
        })?;
        cls.method0("test", |this: &Qt<QObject>| {
            println!("REAL >>>>>>>>>>>>>>>>>>");
            Ok(())
        })?;
        cls.static_fn0("staticTest", || {
            println!("STATIC >>>>>>>>>>>>>>>>>>");
            Ok(())
        })?;
        Ok(())
    }
}

impl Type for Qt<QWidget> {
    const CLASS_NAME: &'static str = "QWidget";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        cls.constructor0(|| unsafe { Ok(Qt::new(QWidget::new_0a())) })?;
        cls.method0("show", |this: &Qt<QWidget>| unsafe {
            this.show();
            Ok(())
        })?;
        cls.method0("hide", |this: &Qt<QWidget>| unsafe {
            this.hide();
            Ok(())
        })?;
        cls.superclass::<Qt<QObject>>()?;
        Ok(())
    }
}

impl Type for Qt<QApplication> {
    const CLASS_NAME: &'static str = "QApplication";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        // cls.static_fn0("exec", || unsafe {
        //     QCoreApplication::exec();
        // })?;
        cls.constructor0(|| unsafe {
            let mut args = QCoreApplicationArgs::new();
            let (argc, argv) = args.get();
            std::mem::forget(args);
            Ok(Qt::new(QApplication::new_2a(argc, argv)))
        })?;
        cls.superclass::<Qt<QObject>>()?;
        Ok(())
    }
}

impl Type for Qt<QMainWindow> {
    const CLASS_NAME: &'static str = "QMainWindow";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        cls.static_fn0("exec", || unsafe {
            QCoreApplication::exec();
            Ok(())
        })?;
        cls.method1(
            "setCentralWidget",
            |this: &Qt<QMainWindow>, child: Virt<Qt<QWidget>>| {
                unsafe {
                    this.set_central_widget(child.as_ptr());
                }
                Ok(())
            },
        )?;
        cls.constructor0(|| unsafe { Ok(Qt::new(QMainWindow::new_0a())) })?;
        cls.superclass::<Qt<QWidget>>()?;
        Ok(())
    }
}

impl Type for Qt<QPushButton> {
    const CLASS_NAME: &'static str = "QPushButton";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        cls.constructor0(|| unsafe { Ok(Qt::new(QPushButton::new())) })?;
        cls.prop(
            "text",
            |this: &Qt<QPushButton>| unsafe { Ok(this.text().to_std_string()) },
            |this: &Qt<QPushButton>, value: String| unsafe {
                let value = QString::from_std_str(value);
                this.set_text(value.as_ref());
                Ok(())
            },
        )?;
        cls.superclass::<Qt<QWidget>>()?;
        Ok(())
    }
}
