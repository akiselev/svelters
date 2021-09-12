use crate::{inner::*, Gtk};

pub(in crate) macro module($modname:ty: $($ty:ty),*$(,)?) {
    impl Module for $modname {
        const MODULE_NAME: &'static str = stringify!($modname);

        #[inline(always)]
        fn define<P>(module: ModuleBuilder<Self, P>) -> Result<ModuleBuilder<Self, P>>
        {
            $(let module = module.class::<Gtk<$ty>>()?;)*
            Ok(module)
        }
    }
}

pub(in crate) macro gtk_enum($name:ty: $($val:literal as $var:path),*$(,)?) {
    impl<'js> rquickjs::IntoJs<'js> for Gtk<$name> {
        fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
            match self.0 {
                $(
                    $var => $val.into_js(ctx),
                )*
                _ => Err(exception!("Uknown value of {}", stringify!($name)))
            }
        }
    }

    impl<'js> rquickjs::FromJs<'js> for Gtk<$name> {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            let val: String = String::from_js(ctx, value)?;
            match val.as_str() {
                $(
                    $val => Ok(Gtk($var)),
                )*
                _ => Err(exception!("\"{}\" is not a valid value for {}", val, stringify!($name)))
            }
        }
    }
}

pub(in crate) macro define (|$builder:ident: $cls:ty| { $($tokens:tt)* }) {
    impl Type for Gtk<$cls> {
        const CLASS_NAME: & 'static str = stringify!($cls);

        unsafe fn metadata() -> &'static mut Metadata {
            static mut META: Metadata = Metadata::new::<Gtk<$cls>>();
            &mut META
        }

        #[inline(always)]
        fn define<P>($builder: TypeBuilder<Self, P>) -> Result<()> {
            let $builder = $builder.class()?;
            $($tokens)*
            Ok(())
        }
    }
}

pub(in crate) macro event {
    ($cls:ident.$name:ident as $slot:ty[$($arg:ident),* ]) => {
        let $cls = $cls.with_event(stringify!($name), |el, func| unsafe {
            el.event(stringify!($name))?
                .subscribe(func)
                .connect(|this: &Self, sub, dc| {
                    let conn = <$slot>::new(this.as_ptr(), move |$($arg ,)*| {
                        tracing::info!("Handling {}::{}", stringify!($cls), stringify!($name));
                        sub.dispatch(());
                    });
                    let disconnect = this.clicked().connect(&conn);
                    dc.with(move |_this: &Self, _sub| {
                        QObject::disconnect_connection(disconnect.as_ref());
                        Ok(())
                    })
                })
        })?;
    },
    ($cls:ident.$name:ident as $slot:ty[$($arg:ident),*]($($var:ident),*)) => {
        let $cls = $cls.with_event(stringify!($name), |el, func| unsafe {
            el.event(stringify!($name))?
                .subscribe(func)
                .connect(|this: &Self, sub, dc| {
                    let conn = <$slot>::new(this.as_ptr(), move |$($arg ,)*| {
                        tracing::info!("Handling {}::{}", stringify!($cls), stringify!($name));

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
        })?;
    }
}

pub(in crate) macro prop {
    ($bld:ident($this:ident: $cls:ty) => $name:ident: $ty:ty { $get:block; $set:block $(;)? }) => {
        let $bld = $bld.prop(
            stringify!($name),
            |$this: &$cls| -> rquickjs::Result<$ty> { unsafe { Ok($get) } },
            |$this: &$cls, $name: $ty| unsafe {
                $set
                Ok(())
            },
        )?;
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident: $ty:ty { $get:expr; $set:expr $(;)? }) => {
        let $bld = $bld.prop(
            stringify!($name),
            |$this: &$cls| -> rquickjs::Result<$ty> { unsafe { Ok($get) } },
            |$this: &$cls, $name: $ty| unsafe {
                { $set }
                Ok(())
            },
        )?;
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident: $ty:ty { $get:block }) => {
        let $bld = $bld.readonly(
            stringify!($name),
            |$this: &$cls| -> rquickjs::Result<$ty> { unsafe { Ok($get) } },
        )?;
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident: $ty:ty { $get:expr $(;)? }) => {
        let $bld = $bld.readonly(
            stringify!($name),
            |$this: &$cls| -> rquickjs::Result<$ty> { unsafe { Ok($get) } },
        )?;
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
    (@num 0) => { ClassBuilder::method0 },
    (@num 1) => { ClassBuilder::method1 },
    (@num 2) => { ClassBuilder::method2 },
    (@num 3) => { ClassBuilder::method3 },
    (@num 4) => { ClassBuilder::method4 },
    (@num 5) => { ClassBuilder::method5 },
    (@num 6) => { ClassBuilder::method6 },
    (@num 7) => { ClassBuilder::method7 },
    (@num 8) => { ClassBuilder::method8 },
    ($bld:ident($this:ident: $cls:ty) => $name:ident[$num:tt]() $(-> $res:ty)? { $func:expr }) => {
        let func = method!(@num $num);
        let $bld = func(
            $bld,
            stringify!($name),
            |$this: &$cls| $(-> rquickjs::Result<$res>)? { unsafe { Ok($func) } },
        )?;
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident[$num:tt]($($arg:ident),*) $(-> $res:ty)? { $func:expr }) => {
        let func = method!(@num $num);
        let $bld = func(
            $bld,
            stringify!($name),
            |$this: &$cls, $($arg),*| $(-> rquickjs::Result<$res>)? {
                unsafe { Ok($func) }
            },
        )?;
    },
    ($bld:ident($this:ident: $cls:ty) => $name:ident[$num:tt]($($arg:ident: $argty:ty),*) $(-> $res:ty)? { $func:expr }) => {
        let func = method!(@num $num);
        let $bld = func(
            $bld,
            stringify!($name),
            |$this: &$cls, $($arg: $argty),*| $(-> rquickjs::Result<$res>)? {
                unsafe { Ok($func) }
            },
        )?;
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
