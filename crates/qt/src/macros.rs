use crate::{impls::containers::Qt, inner::*};

pub(in crate) macro module($modname:ty: $($ty:ty),*$(,)?) {
    impl Module for $modname {
        #[inline(always)]
        fn define<P>(module: &mut ModuleBuilder<Self, P>) -> Result<()>
        where
           crate::QT: Framework,
        {
            $(module.class::<Qt<$ty>>()?;)*
            Ok(())
        }
    }
}

pub(in crate) macro qdefine(|$builder:ident: $cls:ty| { $($tokens:tt)* }) {
    impl Type for Q<$cls> {
        const CLASS_NAME: & 'static str = stringify!($cls);

        unsafe fn metadata() -> &'static mut Metadata {
            static mut META: Metadata = Metadata::new();
            &mut META
        }

        #[inline(always)]
        fn define<P>($builder: &mut TypeBuilder<Self, P>) -> Result<()> {
            $($tokens)*
            Ok(())
        }
    }
}

pub(in crate) macro define (|$builder:ident: $cls:ty| { $($tokens:tt)* }) {
    impl Type for Qt<$cls> {
        const CLASS_NAME: & 'static str = stringify!($cls);

        unsafe fn metadata() -> &'static mut Metadata {
            static mut META: Metadata = Metadata::new();
            &mut META
        }

        #[inline(always)]
        fn define<P>($builder: &mut TypeBuilder<Self, P>) -> Result<()> {
            $($tokens)*
            Ok(())
        }
    }
}

pub(in crate) macro event {
    ($cls:ident.$name:ident as $slot:ty[$($arg:ident),* ]) => {
        $cls.with_event(stringify!($name), |el, func| unsafe {
            el.event(stringify!($name))?
                .subscribe(func)
                .connect(|this: &Self, sub, dc| {
                    let conn = <$slot>::new(this.as_ptr(), move |$($arg ,)*| {
                        tracing::info!("Handling QAbstractButton::clicked");
                        sub.dispatch(());
                    });
                    let disconnect = this.clicked().connect(&conn);
                    dc.with(move |_this: &Self, _sub| {
                        QObject::disconnect_connection(disconnect.as_ref());
                        Ok(())
                    })
                })
        })?
    },
    ($cls:ident.$name:ident as $slot:ty[$($arg:ident),*]($($var:ident),*)) => {
        $cls.with_event(stringify!($name), |el, func| unsafe {
            el.event(stringify!($name))?
                .subscribe(func)
                .connect(|this: &Self, sub, dc| {
                    let conn = <$slot>::new(this.as_ptr(), move |$($arg ,)*| {
                        tracing::info!("Handling QAbstractButton::clicked");

                        sub.dispatch((
                            $( { $var } ),*
                            // $( { ERROR_Argument_Does_Not_Implement_FromJs::assert(FromJsErr($var)) } ),*
                        ));
                    });
                    let disconnect = this.clicked().connect(&conn);
                    dc.with(move |_this: &Self, _sub| {
                        QObject::disconnect_connection(disconnect.as_ref());
                        Ok(())
                    })
                })
        })?
    }
}

pub(in crate) macro prop {
    ($bld:ident($this:ident: $cls:ty) => $name:ident: $ty:ty { $get:block; $set:block $(;)? }) => {
        $bld.prop(
            stringify!($name),
            |$this: &Qt<$cls>| -> rquickjs::Result<$ty> { unsafe { Ok($get) } },
            |$this: &Qt<$cls>, $name: $ty| unsafe {
                $set
                Ok(())
            },
        )?
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident: $ty:ty { $get:expr; $set:expr $(;)? }) => {
        $bld.prop(
            stringify!($name),
            |$this: &Qt<$cls>| -> rquickjs::Result<$ty> { unsafe { Ok($get) } },
            |$this: &Qt<$cls>, $name: $ty| unsafe {
                { $set }
                Ok(())
            },
        )?
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident: $ty:ty { $get:block }) => {
        $bld.readonly(
            stringify!($name),
            |$this: &Qt<$cls>| -> rquickjs::Result<$ty> { unsafe { Ok($get) } },
        )?
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident: $ty:ty { $get:expr $(;)? }) => {
        $bld.readonly(
            stringify!($name),
            |$this: &Qt<$cls>| -> rquickjs::Result<$ty> { unsafe { Ok($get) } },
        )?
    }
}

pub(in crate) macro props {
    ($bld:ident($this:ident: $cls:ty) { $($name:ident: $ty:ty { $($tt:tt)* }; )* }) => {
        $(
            prop!($bld($this: $cls) => $name: $ty { $($tt)* });
        )*
    }
}

pub(in crate) macro method {
    (@num 0) => { TypeBuilder::method0 },
    (@num 1) => { TypeBuilder::method1 },
    (@num 2) => { TypeBuilder::method2 },
    (@num 3) => { TypeBuilder::method3 },
    (@num 4) => { TypeBuilder::method4 },
    (@num 5) => { TypeBuilder::method5 },
    (@num 6) => { TypeBuilder::method6 },
    (@num 7) => { TypeBuilder::method7 },
    (@num 8) => { TypeBuilder::method8 },
    ($bld:ident($this:ident: $cls:ty) => $name:ident[$num:tt]() $(-> $res:ty)? { $func:expr }) => {
        let func = method!(@num $num);
        func(
            $bld,
            stringify!($name),
            |$this: &Qt<$cls>| $(-> rquickjs::Result<$res>)? { unsafe { Ok($func) } },
        )?
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident[$num:tt]($($arg:ident),*) $(-> $res:ty)? { $func:expr }) => {
        let func = method!(@num $num);
        func(
            $bld,
            stringify!($name),
            |$this: &Qt<$cls>, $($arg),*| $(-> rquickjs::Result<$res>)? {
                unsafe { Ok($func) }
            },
        )?
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident[$num:tt]($($arg:ident: $argty:ty),*) $(-> $res:ty)? { $func:expr }) => {
        let func = method!(@num $num);
        func(
            $bld,
            stringify!($name),
            |$this: &Qt<$cls>, $($arg: $argty),*| $(-> rquickjs::Result<$res>)? {
                unsafe { Ok($func) }
            },
        )?
    },
}

pub(in crate) macro methods {
    ($bld:ident($this:ident: $cls:ty) { $($name:ident[$num:tt]() $(-> $ty:ty)? { $($tt:tt)* };)* }) => {
        $(
            method!($bld($this: $cls) => $name[$num]() $(-> $ty)? { $($tt)* });
        )*
    },
    ($bld:ident($this:ident: $cls:ty) { $($name:ident[$num:tt]($($arg:ident),*) $(-> $ty:ty)? { $($tt:tt)* }; )* }) => {
        $(
            method!($bld($this: $cls) => $name[$num]($($arg),*) $(-> $ty)? { $($tt)* });
        )*
    },
    ($bld:ident($this:ident: $cls:ty) { $($name:ident[$num:tt]($($arg:ident: $argty:ty),*) $(-> $ty:ty)? { $($tt:tt)* }; )* }) => {
        $(
            method!($bld($this: $cls) => $name[$num]($($arg: $argty),*) $(-> $ty)? { $($tt)* });
        )*
    }
}

pub(in crate) macro qprop {
    ($bld:ident($this:ident: $cls:ty) => $name:ident: $ty:ty { $get:block; $set:block $(;)? }) => {
        $bld.prop(
            stringify!($name),
            |$this: &Q<$cls>| -> rquickjs::Result<$ty> { unsafe { Ok($get) } },
            |$this: &Q<$cls>, $name: $ty| unsafe {
                $set
                Ok(())
            },
        )?
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident: $ty:ty { $get:expr; $set:expr $(;)? }) => {
        $bld.prop(
            stringify!($name),
            |$this: &Q<$cls>| -> rquickjs::Result<$ty> { unsafe { Ok($get) } },
            |$this: &Q<$cls>, $name: $ty| unsafe {
                { $set }
                Ok(())
            },
        )?
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident: $ty:ty { $get:block }) => {
        $bld.readonly(
            stringify!($name),
            |$this: &Q<$cls>| -> rquickjs::Result<$ty> { unsafe { Ok($get) } },
        )?
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident: $ty:ty { $get:expr $(;)? }) => {
        $bld.readonly(
            stringify!($name),
            |$this: &Q<$cls>| -> rquickjs::Result<$ty> { unsafe { Ok($get) } },
        )?
    }
}

pub(in crate) macro qprops {
    ($bld:ident($this:ident: $cls:ty) { $($name:ident: $ty:ty { $($tt:tt)* }; )* }) => {
        $(
            prop!($bld($this: $cls) => $name: $ty { $($tt)* });
        )*
    }
}

pub(in crate) macro qmethod {
    (@num 0) => { TypeBuilder::method0 },
    (@num 1) => { TypeBuilder::method1 },
    (@num 2) => { TypeBuilder::method2 },
    (@num 3) => { TypeBuilder::method3 },
    (@num 4) => { TypeBuilder::method4 },
    (@num 5) => { TypeBuilder::method5 },
    (@num 6) => { TypeBuilder::method6 },
    (@num 7) => { TypeBuilder::method7 },
    (@num 8) => { TypeBuilder::method8 },
    ($bld:ident($this:ident: $cls:ty) => $name:ident[$num:tt]() $(-> $res:ty)? { $func:expr }) => {
        let func = qmethod!(@num $num);
        func(
            $bld,
            stringify!($name),
            |$this: &Q<$cls>| $(-> rquickjs::Result<$res>)? { unsafe { Ok($func) } },
        )?
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident[$num:tt]($($arg:ident),*) $(-> $res:ty)? { $func:expr }) => {
        let func = qmethod!(@num $num);
        func(
            $bld,
            stringify!($name),
            |$this: &Q<$cls>, $($arg),*| $(-> rquickjs::Result<$res>)? {
                unsafe { Ok($func) }
            },
        )?
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident[$num:tt]($($arg:ident: $argty:ty),*) $(-> $res:ty)? { $func:expr }) => {
        let func = qmethod!(@num $num);
        func(
            $bld,
            stringify!($name),
            |$this: &Q<$cls>, $($arg: $argty),*| $(-> rquickjs::Result<$res>)? {
                unsafe { Ok($func) }
            },
        )?
    },
}

pub(in crate) macro qmethods {
    ($bld:ident($this:ident: $cls:ty) { $($name:ident[$num:tt]() $(-> $ty:ty)? { $($tt:tt)* };)* }) => {
        $(
            qmethod!($bld($this: $cls) => $name[$num]() $(-> $ty)? { $($tt)* });
        )*
    },
    ($bld:ident($this:ident: $cls:ty) { $($name:ident[$num:tt]($($arg:ident),*) $(-> $ty:ty)? { $($tt:tt)* }; )* }) => {
        $(
            qmethod!($bld($this: $cls) => $name[$num]($($arg),*) $(-> $ty)? { $($tt)* });
        )*
    },
    ($bld:ident($this:ident: $cls:ty) { $($name:ident[$num:tt]($($arg:ident: $argty:ty),*) $(-> $ty:ty)? { $($tt:tt)* }; )* }) => {
        $(
            qmethod!($bld($this: $cls) => $name[$num]($($arg: $argty),*) $(-> $ty)? { $($tt)* });
        )*
    }
}

pub(in crate) macro exc {
    (cast $(to)? $id:ident) => {
        svelters_core::exception!("Could not cast as {}", stringify!($id))
    },
    () => { svelters_core::exception!("UNKNOWN ERROR") },
    ($($tt:tt)*) => { svelters_core::exception!($($tt)*) },
}

pub struct FromJsErr<T>(pub T);
pub trait ERROR_Argument_Does_Not_Implement_FromJs<T>: Sized {
    fn assert(self) -> T;
}
impl<T: for<'js> FromJs<'js>> ERROR_Argument_Does_Not_Implement_FromJs<T> for FromJsErr<T> {
    fn assert(self) -> T {
        self.0
    }
}

pub struct IntoJsErr<T>(pub T);
pub trait ERROR_Argument_Does_Not_Implement_IntoJs<T>: Sized {
    fn assert(self) -> T;
}
impl<T: for<'js> IntoJs<'js>> ERROR_Argument_Does_Not_Implement_IntoJs<T> for IntoJsErr<T> {
    fn assert(self) -> T {
        self.0
    }
}

macro assert_impl {
    ($trait:path: $($ty:ty),+) => {{
        struct Helper<T>(T);
        trait AssertImpl { fn assert() {} }
        impl<T: $trait> AssertImpl for Helper<T> {}
        $(
            Helper::<$ty>::assert();
         )+
    }},
    (!$trait:path: $($ty:ty),+) => {{
        struct Helper<T>(T);
        trait AssertImpl { fn assert() {} }
        impl<T: $trait> AssertImpl for Helper<T> {}
        trait AssertNotImpl { fn assert() {} }
        $(
            impl AssertNotImpl for Helper<$ty> {}
            Helper::<$ty>::assert();
         )+
    }},
    ($trait:path: $($ty:ty,)+) => (assert_impl!($trait: $($ty),+)),
    (!$trait:path: $($ty:ty,)+) => (assert_impl!(!$trait: $($ty),+)),
}
