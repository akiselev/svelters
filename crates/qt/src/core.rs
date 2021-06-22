use std::marker::PhantomData;

pub trait Ret<A> {
    type T<X>: Fn(A) -> X;
}

pub trait Const0 {
    type FN<R>: Fn() -> R;
}

impl<R, F: Fn() -> R> Const0 for F {
    type FN<R> = F;
}

pub trait Const1 {
    type FN<A, R>: Fn(A) -> R;
}

pub trait Const2 {
    type FN<A, B, R>: Fn(A, B) -> R;
}

pub trait Constructor {}

pub struct Func0<T: Const0> {
    func: T,
}

pub struct Func1<T: Const1> {
    func: T,
}

pub struct Func2<T: Const2> {
    func: T,
}

#[macro_export]
macro_rules! FN {
    ($e:expr) => {
        spez::spez! {
            for x = $e;
            match <T: Const0> T -> Func0<T> {
                println!("Const0");
                Func0::new(x)
            }
            match <T: Const1> T -> Func1<T> {
                println!("Const1");
                Func1::new(x)
            }
            match <T: Const2> T -> Func2<T> {
                println!("Const2");
                Func2::new(x)
            }
            match<T> T {
                impl_error::<Constructor, _>(x);
            }
        }
    };
}

pub trait ERROR___IS_NOT_A_VALID_<T> {}
pub fn impl_error<T, E: ERROR___IS_NOT_A_VALID_<T>>(input: E) {}
