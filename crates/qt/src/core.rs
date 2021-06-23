use std::{borrow::Cow, marker::PhantomData};

use rquickjs::{Ctx, Error, Result, Value};

pub enum TypeKind {
    Primitive,
    Struct,
    ForeignStruct,
    Class,
}

pub trait TypeDef {
    fn type_name(&self) -> Cow<'static, str>;
}

// pub trait BuildClass {
//     fn build(&self, name: &str) -> impl TypeDef;
// }

// #[macro_export]
// macro_rules! FN {
//     ($e:expr) => {
//         spez::spez! {
//             for x = $e;
//             match <T: Const0> T -> Func0<T> {
//                 println!("Const0");
//                 Func0::new(x)
//             }
//             match <T: Const1> T -> Func1<T> {
//                 println!("Const1");
//                 Func1::new(x)
//             }
//             match <T: Const2> T -> Func2<T> {
//                 println!("Const2");
//                 Func2::new(x)
//             }
//             match<T> T {
//                 impl_error::<Constructor, _>(x);
//             }
//         }
//     };
// }

// pub trait ERROR___IS_NOT_A_VALID_<T> {}
// pub fn impl_error<T, E: ERROR___IS_NOT_A_VALID_<T>>(input: E) {}

// pub trait Type {
//     fn type_name() -> Cow<'static, str>;
// }

// trait BuildArgs<A> {
//     fn get_args<'js>(ctx: Ctx<'js>, args: Vec<Value<'js>>) -> Result<A>;
// }

// trait Constructor {
//     type Args<A>: BuildArgs<A>;
//     type Return<'js, F>: ToJs<'js>;
// }
