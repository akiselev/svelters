#![feature(generic_associated_types)]
#![feature(impl_trait_in_bindings)]
#![feature(in_band_lifetimes)]
use std::borrow::Cow;

use frunk::hlist;
use qt_core::{
    cpp_core::{CastFrom, CppBox, CppDeletable, Ptr, StaticUpcast},
    QBox, QCoreApplicationArgs, QObject, QPtr, QString,
};
use qt_widgets::{QApplication, QMainWindow, QPushButton, QWidget};
use rquickjs::{Ctx, Error, Result, Value};
use static_assertions::assert_impl_all;
use svelters_qt::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct QT;

#[derive(Debug, Clone, Copy)]
pub struct AsRefWrapper<T>(T);

impl<T> std::ops::Deref for AsRefWrapper<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: CppDeletable> AsRef<T> for AsRefWrapper<CppBox<T>> {
    fn as_ref(&self) -> &T {
        &*self
    }
}

impl<T: CppDeletable + StaticUpcast<QObject>> AsRef<T> for AsRefWrapper<QBox<T>> {
    fn as_ref(&self) -> &T {
        &*self
    }
}

static qt: Core<QT> = Core::new(QT);

static QOBJECT: impl Instantiate =
    qt.class::<QObject, _>(hlist![qt.init(|| unsafe { QObject::new_0a() })]);
static QWIDGET: impl Instantiate =
    qt.class::<QWidget, _>(hlist![qt.init(|| unsafe { QWidget::new_0a() })]);
static QMAINWINDOW: impl Instantiate =
    qt.class::<QMainWindow, _>(hlist![qt.init(|| unsafe { QMainWindow::new_0a() })]);
static QPUSHBUTTOn: impl Instantiate =
    qt.class::<QPushButton, _>(hlist![qt.init(|| unsafe { QPushButton::new() })]);

impl Framework for QT {}

impl<T> Container<QT, T> for CppBox<T>
where
    T: CppDeletable,
    AsRefWrapper<CppBox<T>>: AsRef<T>,
{
    type Output<X> = AsRefWrapper<CppBox<T>>;

    fn wrap(self) -> Self::Output<Self> {
        AsRefWrapper(self)
    }
}

impl<T> Container<QT, T> for QBox<T>
where
    T: CppDeletable + StaticUpcast<QObject>,
{
    type Output<X> = AsRefWrapper<QBox<T>>;

    fn wrap(self) -> Self::Output<Self> {
        AsRefWrapper(self)
    }
}

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
    Ptr<QWidget>: CastFrom<<T as Cast<Ptr<QWidget>>>::Output<'a, Ptr<QWidget>>>, // Ptr<QWidget>: CastFrom<<T as svelters_qt::Cast<QWidget>>::Output<'a, QWidget>>,
{
    let w = win.cast::<'a>()?;
    let w2 = wid.cast::<'a>()?;
    QMainWindow::set_central_widget(w.as_ref(), w2);
    None
}

#[rquickjs::bind(module, public)]
#[quickjs(bare)]
#[allow(non_upper_case_globals)]
pub mod native_module {
    use svelters_qt::prelude::*;
    pub const n: i32 = 123;
    pub const s: &str = "abc";
    pub fn f(a: f64, b: f64) -> f64 {
        (a + b) * 0.5
    }

    #[derive(HasRefs)]
    #[quickjs(has_refs)]
    pub struct Test {
        #[quickjs(has_refs)]
        x: Persistent<Value<'static>>,
        y: String,
    }

    impl Test {
        #[quickjs(constructor)]
        pub fn new<'js>(x: Persistent<Value<'static>>, y: String) -> Result<Test> {
            Ok(Test { x, y })
        }

        pub fn format<'js>(&'js self, ctx: Ctx<'js>, index: u64) -> Result<String> {
            let x = Persistent::restore(self.x.clone(), ctx)?
                .as_string()
                .unwrap()
                .to_string()?;
            Ok(format!("{}: {}", x.as_str(), index))
        }

        pub fn new_static(index: u64) -> Result<String> {
            Ok(format!("{}: ", index))
        }

        #[quickjs(rename = "x", get)]
        pub fn get<'js>(&'js self, ctx: Ctx<'js>) -> Result<Value<'js>> {
            Ok(Persistent::restore(self.x.clone(), ctx)?)
        }

        #[quickjs(rename = "x", set)]
        pub fn set<'js>(&'js mut self, ctx: Ctx<'js>, x: u64) {
            panic!("....")
        }

        #[quickjs(rename = "new_static")]
        pub fn new_static2<'js>(x: String) {
            panic!("....")
        }
    }
}
