use crate::inner::*;

pub macro prop(|$this:ident: $ty:ty| $name:ident: $prop:ty {
    $getter:expr;
    $setter:expr;
}) {
    $this.prop(
        stringify!($name),
        crate::inner::GtkProp::new::<$ty>(
            stringify!($ty),
            stringify!($name),
            |ctx: rquickjs::Ctx, $this: &$ty| {
                let res: $prop = { $getter };
                res
            },
            |ctx: rquickjs::Ctx, $this: &$ty, $name: $prop| {
                {
                    $setter
                }
                Ok(())
            },
        ),
    );
}

pub macro props(|$this:ident: $ty:ty| {
    $(
        $name:ident: $prop:ty {
            $($tt:tt)*
        }$(;)?
    )*
}) {
    $(
        crate::macros::prop!(|$this: $ty| $name: $prop {
            $($tt)*
        });
    )*
}

pub(in crate) macro method {
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
