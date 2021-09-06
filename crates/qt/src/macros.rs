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

pub(in crate) macro define(|$builder:ident: $cls:ty| { $($tokens:tt)* }) {
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

pub(in crate) macro event($cls:ident.$name:ident as $slot:ty[$($arg:ident ,)*]) {
    $cls.with_event(stringify!($name), |el, func| unsafe {
        el.event(stringify!($name))?
            .subscribe(func)
            .connect(|this: &Self, sub, dc| {
                let conn = <$slot>::new(this.as_ptr(), move |$($arg ,)*| {
                    tracing::info!("Handling QAbstractButton::clicked");
                    sub.dispatch(($($arg),*));
                });
                let disconnect = this.clicked().connect(&conn);
                dc.with(move |_this: &Self, _sub| {
                    QObject::disconnect_connection(disconnect.as_ref());
                    Ok(())
                })
            })
    })?
}

pub(in crate) macro exc {
    (cast $(to)? $id:ident) => {
        svelters_core::exception!("Could not cast as {}", stringify!($id))
    },
    () => { svelters_core::exception!("UNKNOWN ERROR") },
    ($($tt:tt)*) => { svelters_core::exception!($($tt)*) },
}

// #[macro_export(crate)]
// macro_rules! define {
//     ($builder:ident: $cls:ty $block:block) => {
//         impl svelters_core::prelude::Type for Qt<$cls> {
//             const CLASS_NAME: &'static str = " stringify!($cls)";

//             unsafe fn metadata() -> &'static mut Metadata {
//                 static mut META: Metadata = Metadata::new();
//                 &mut META
//             }

//             #[inline(always)]
//             fn define<P>($builder: &mut TypeBuilder<Self, P>) -> Result<()> {
//                 $block
//                 Ok(())
//             }
//         }
//     };
// }
