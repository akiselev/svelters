use static_assertions::assert_impl_all;

use crate::inner::*;

pub trait Func<A> {
    type Args;
    type Output;

    fn try_call<'js>(&self, args: Self::Args) -> Self::Output;
}

// // implement the above for functions of varying arities
macro_rules! define_ext_fn {
    ($( $(($argn:ident $arg:ident))* => $res:ident ),+) => (
        $(
            impl <F,$res,$($arg),*> Func<($res, $($arg),*)> for F
            where
              F: Fn($($arg),*) -> $res,
            {
                type Args = ($($arg),*);
                type Output = $res;

                fn try_call(&self, ($($argn),*): Self::Args) -> Self::Output {
                    (*self)($($argn),*)
                }
            }
        )+
    )
}

macro_rules! define_unsafe_fn {
    ($( $(($argn:ident $arg:ident))* => $res:ident ),+) => (
        $(
            impl <$res,$($arg),*> Func<($res, $($arg),*)> for unsafe fn($($arg),*) -> $res
            {
                type Args = ($($arg),*);
                type Output = $res;

                fn try_call(&self, ($($argn),*): Self::Args) -> Self::Output {
                    unsafe { (*self)($($argn),*) }
                }
            }

            impl <$res,$($arg),*> Func<($res, $($arg),*)> for &unsafe fn($($arg),*) -> $res
            {
                type Args = ($($arg),*);
                type Output = $res;

                fn try_call(&self, ($($argn),*): Self::Args) -> Self::Output {
                    unsafe { (*self)($($argn),*) }
                }
            }

            impl <$res,$($arg),*> Func<($res, $($arg),*)> for &mut unsafe fn($($arg),*) -> $res
            {
                type Args = ($($arg),*);
                type Output = $res;

                fn try_call(&self, ($($argn),*): Self::Args) -> Self::Output {
                    unsafe { (*self)($($argn),*) }
                }
            }
        )+
    )
}

define_ext_fn!(
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

define_unsafe_fn!(
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
