use crate::inner::*;

pub trait AsFunc<A> {
    type Output;

    fn as_func(self) -> Self::Output;
}

macro_rules! define_as_func {
    ($( $(($argn:ident $arg:ident))* => $res:ident ),+) => (
        $(
            impl <$res,$($arg),*> AsFunc<($res, $($arg),*)> for &unsafe fn($($arg),*) -> $res where Self: 'static
            {
                type Output = impl Func<($res, $($arg),*)>;

                fn as_func(self) -> Self::Output {
                    move |$($argn: $arg),*| unsafe { (*self)($($argn),*) }
                }
            }

        )+
    )
}

define_as_func!(
    => T1,
    (t1 T1) => T2,
    (t1 T1) (t2 T2) => T3,
    (t1 T1) (t2 T2) (t3 T3) => T4,
    (t1 T1) (t2 T2) (t3 T3) (t4 T4) => T5,
    (t1 T1) (t2 T2) (t3 T3) (t4 T4) (t5 T5) => T6,
    (t1 T1) (t2 T2) (t3 T3) (t4 T4) (t5 T5) (t6 T6) => T7,
    (t1 T1) (t2 T2) (t3 T3) (t4 T4) (t5 T5) (t6 T6) (t7 T7) => T8,
    (t1 T1) (t2 T2) (t3 T3) (t4 T4) (t5 T5) (t6 T6) (t7 T7) (t8 T8) => T9,
    (t1 T1) (t2 T2) (t3 T3) (t4 T4) (t5 T5) (t6 T6) (t7 T7) (t8 T8) (t9 T9) => T10
);

pub fn extfn<A, F>(func: F) -> impl Func<A>
where
    F: AsFunc<A>,
    <F as AsFunc<A>>::Output: Func<A>,
{
    AsFunc::<A>::as_func(func)
}
