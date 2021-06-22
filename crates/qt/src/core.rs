use std::marker::PhantomData;

pub enum NodeKind {
    Text { text: String },
    Widget {},
    Slot { name: String },
}

pub struct Node {}

pub trait RT<T> {}

// pub trait FN0 {
//     type Args;
//     type Ret;
// }
// pub trait FN1 {
//     type Args;
//     type Ret;
// }
// pub trait FN2 {
//     type Args;
//     type Ret;
// }
// pub trait FN3 {
//     type Args;
//     type Ret;
// }
// pub trait FN4 {
//     type Args;
//     type Ret;
// }

// impl<R, T: Fn() -> R> FN0 for T {
//     type Args = ();
//     type Ret = R;
// }

// impl<A, R, T: Fn(A) -> R> FN2 for T {
//     type Args = (A);
//     type Ret = R;
// }

// impl<A, B, R, T: Fn(A, B) -> R> FN2 for T {
//     type Args = (A, B);
//     type Ret = R;
// }

// impl<A, B, C, R, T: Fn(A, B, C) -> R> FN3 for T {
//     type Args = (A, B, C);
//     type Ret = R;
// }

// impl<A, B, C, D, R, T: Fn(A, B, C, D) -> R> FN4 for T {
//     type Args = (A, B, C, D);
//     type Ret = R;
// }

pub struct Arg<T>(PhantomData<T>);
impl<T> Arg<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

pub struct Const0<R, F: Fn() -> R> {
    func: F,
}

impl<R, F: Fn() -> R> Const0<R, F> {
    pub fn new(func: F) -> Self {
        Self { func }
    }
}
pub struct Const1<A, R, F: Fn(A) -> R> {
    arg1: Arg<A>,
    func: F,
}

impl<A, R, F: Fn(A) -> R> Const1<A, R, F> {
    pub fn new(func: F) -> Self {
        Self {
            arg1: Arg::new(),
            func,
        }
    }
}
pub struct Const2<A, B, R, F: Fn(A, B) -> R> {
    arg1: Arg<A>,
    arg2: Arg<B>,
    func: F,
}
impl<A, B, R, F: Fn(A, B) -> R> Const2<A, B, R, F> {
    pub fn new(func: F) -> Self {
        Self {
            arg1: Arg::new(),
            arg2: Arg::new(),
            func,
        }
    }
}
pub struct Const3<A, B, C, R, F: Fn(A, B, C) -> R> {
    arg1: Arg<A>,
    arg2: Arg<B>,
    arg3: Arg<C>,
    func: F,
}
impl<A, B, C, R, F: Fn(A, B, C) -> R> Const3<A, B, C, R, F> {
    pub fn new(func: F) -> Self {
        Self {
            arg1: Arg::new(),
            arg2: Arg::new(),
            arg3: Arg::new(),
            func,
        }
    }
}
pub struct Const4<A, B, C, D, R, F: Fn(A, B, C, D) -> R> {
    arg1: Arg<A>,
    arg2: Arg<B>,
    arg3: Arg<C>,
    arg4: Arg<D>,
    func: F,
}
impl<A, B, C, D, R, F: Fn(A, B, C, D) -> R> Const4<A, B, C, D, R, F> {
    pub fn new(func: F) -> Self {
        Self {
            arg1: Arg::new(),
            arg2: Arg::new(),
            arg3: Arg::new(),
            arg4: Arg::new(),
            func,
        }
    }
}

pub trait Constructor {}

// #[macro_export]
// macro_rules! FN {
//     ($e:expr) => {
//         spez::spez! {
//             for x = $e;
//             match <R, F: Fn() -> R> F -> Const0<R, F> {
//                 println!("Const<R, F>");
//                 Const0::new(x)
//             }
//             match <F: Fn(A) -> R> F -> Const1<A, R, F> {
//                 println!("Const<A, R, F>");
//                 Const1::new(x)
//             }
//             match <A, B, R, F: Fn(A, B) -> R> F -> Const2<A, B, R, F> {
//                 println!("Const<A, B, R, F>");
//                 Const2::new(x)
//             }
//             match <A, B, C, R, F: Fn(A, B, C) -> R> F -> Const3<A, B, C, R, F> {
//                 println!("Const<A, B, C, R, F>");
//                 Const3::new(x)
//             }
//             match <A, B, C, D, R, F: Fn(A, B, C, D) -> R> F -> Const4<A, B, C, D, R, F> {
//                 println!("Const<A, B, C, D, R, F>");
//                 Const4::new(x)
//             }
//             match<T> T {
//                 impl_error::<Constructor, _>(x);
//             }
//         }
//     };
// }

pub trait ERROR___IS_NOT_A_VALID_<T> {}
pub fn impl_error<T, E: ERROR___IS_NOT_A_VALID_<T>>(input: E) {}
