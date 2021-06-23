use std::borrow::Cow;

use frunk::hlist;
use qt_core::{
    cpp_core::{CastFrom, Ptr},
    QCoreApplicationArgs, QObject, QPtr, QString,
};
use qt_widgets::{QApplication, QMainWindow, QPushButton, QWidget};
use rquickjs::{Ctx, Error, Result, Value};
use svelters_qt::*;

fn main() {
    unsafe {
        let mut args = QCoreApplicationArgs::new();
        let (argc, argv) = args.get();
        let app = {
            let app = QApplication::new_2a(argc, argv);
            app
        };
        let win = QMainWindow::new_0a();
        let btn = QPushButton::new();
        let text = QString::from_std_str("Test");
        add_widget(&win, &btn);
        btn.set_text(&text);
        win.show();
        QApplication::exec();
        // let
    }
}

unsafe fn add_widget<'a, W: 'a, T: 'a>(win: &'a W, wid: &'a T) -> Option<()>
where
    W: Cast<Ptr<QMainWindow>>,
    T: Cast<Ptr<QWidget>>,
    Ptr<QWidget>: CastFrom<<T as svelters_qt::Cast<Ptr<QWidget>>>::Output<'a, Ptr<QWidget>>>, // Ptr<QWidget>: CastFrom<<T as svelters_qt::Cast<QWidget>>::Output<'a, QWidget>>,
{
    let w = win.cast::<'a>()?;
    let w2 = wid.cast::<'a>()?;
    QMainWindow::set_central_widget(w.as_ref(), w2);
    None
}

unsafe fn create_app() -> qt_core::QBox<QApplication> {
    let mut args = QCoreApplicationArgs::new();
    let (argc, argv) = args.get();
    let app = QApplication::new_2a(argc, argv);
    app
}

// #[derive(Debug, Copy, Clone)]
// pub struct Qt(impl Upcaster<String>);
