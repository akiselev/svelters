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
    QtCore,
};

define!(|cls: QMainWindow| {
    cls.static_fn0("exec", || unsafe {
        QCoreApplication::exec();
        Ok(())
    })?
    .method1(
        "setCentralWidget",
        |this: &Qt<QMainWindow>, child: Virt<Qt<QWidget>>| {
            tracing::info!(" LOOK AT THIS");
            unsafe {
                this.set_central_widget(child.as_ptr());
            }
            Ok(())
        },
    )?
    .constructor0(|| unsafe { Ok(Qt::new(QMainWindow::new_0a())) })?
    .as_element("qMainWindow", || unsafe {
        Ok(Qt::new(QMainWindow::new_0a()))
    })?
    .superclass::<Qt<QWidget>>()?
    .with_slot(BasicSlot::new(
        |node, child| {
            let this = node.cast_as::<Self>().unwrap();
            let widget = child.cast_as::<Qt<QWidget>>()?;
            unsafe { this.set_central_widget(widget.as_ptr()) };
            node.append_node(child.clone());
            Ok(())
        },
        |node, child, anchor| {
            if let Some(anchor) = anchor {
                Err(exc!("QMainWindow only allows one central widget"))
            } else {
                let this = node.cast_as::<Self>().unwrap();
                let widget = child.cast_as::<Qt<QWidget>>()?;
                unsafe { this.set_central_widget(widget.as_ptr()) };
                node.append_node(child.clone());
                Ok(())
            }
        },
        |node, child| {
            let this = node.cast_as::<Self>().unwrap();
            let widget = child.cast_as::<Qt<QWidget>>()?;
            unsafe { this.take_central_widget() };
            node.remove_node(child.clone());
            Ok(())
        },
    ))?;
});
