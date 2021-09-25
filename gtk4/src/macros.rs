use crate::inner::*;

pub macro prop(|$this:ident: $ty:ty| $name:ident: $prop:ty {
    $getter:expr;
    $setter:expr;
}) {
    $this.prop(
        stringify!($name),
        crate::GtkProp::new::<$ty>(
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
