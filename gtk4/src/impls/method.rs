use crate::inner::*;

pub struct GtkMethod<IMPL, FUNC, ARGS> {
    func: FUNC,
    name: &'static str,
    interface: &'static str,
    marker: PhantomData<(ARGS, IMPL)>,
}

impl<FUNC, ARGS> GtkMethod<(), FUNC, ARGS> {
    #[inline(always)]
    pub fn new<IMPL>(
        interface: &'static str,
        name: &'static str,
        func: FUNC,
    ) -> GtkMethod<IMPL, FUNC, ARGS>
    where
        IMPL: 'static,
        FUNC: Fn<ARGS>,
    {
        GtkMethod {
            name,
            func,
            interface,
            marker: PhantomData,
        }
    }
}

macro_rules! tup {
    ($($($arg:ident)*,)*) => {
        $(
            #[allow(unused_variables)]
            impl<IMPL, FUNC, $($arg,)*> IMethod for GtkMethod<IMPL, FUNC, (&IMPL, $($arg,)*)>
            where
                IMPL: 'static ,
                FUNC: 'static + for<'a> Fn<(&'a IMPL, $($arg,)*), Output: for<'js> IntoJs<'js>>,
                $($arg: 'static + for<'js> FromJs<'js>,)*
            {
                fn call<'js, 'a>(
                    &self,
                    ctx: Ctx<'js>,
                    element: &'a dyn IElement,
                    args: Vec<Value<'js>>,
                ) -> svelters_core::prelude::Result<Value<'js>> {
                    let args_len = args.len();
                    tuple_match_size(args_len, {0 $(+ tup!(@one $arg))*})?;
                    let this = element.query_ref::<Upcast<IMPL>>().ok_or_else(|| exception!("Could not cast element to {}", std::any::type_name::<IMPL>()))?;
                    (self.func)(
                        &this.upcast(),
                        $({ let arg: $arg = FromJs::from_js(ctx, args.get(tup!(@idx $arg)).cloned().unwrap())?; arg },)*
                    ).into_js(ctx)
                }
            }

            // #[allow(unused_variables)]
            // impl<'a, 'b: 'a, BASE, FUNC, CLS, RET, $($arg,)*> Implements<InitProto<'a, 'b, BASE>>
            //     for members::Method<CLS, (CLS, $($arg,)*), RET, FUNC>
            // where
            //     CLS: Type,
            //     BASE: Type + Upcast<CLS>,
            //     $($arg: 'static + FromJs<'b>,)*
            //     RET: 'static + for<'js> IntoJs<'js>,
            //     FUNC: 'static + for<'x> Fn(&'x CLS, $($arg,)*) -> Result<RET>,
            // {
            //     #[inline(always)]
            //     fn build_phase(self, phase: InitProto<'a, 'b, BASE>) -> Result<InitProto<'a, 'b, BASE>> {
            //         phase.obj.set(
            //             self.name,
            //             rquickjs::Func::new(
            //                 self.name,
            //                 rquickjs::Method(
            //                     move |this: JsClass<'b, Obj<BASE>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
            //                         let args = args.into_inner();
            //                         let args_len = args.len();
            //                         tuple_match_size(args_len, {0 $(+ tup!(@one $arg))*})?;
            //                         let obj: &Obj<BASE> = this.as_ref();
            //                         let ptr = obj.borrow().unwrap();
            //                         (self.func)(
            //                             &ptr.upcast(),
            //                             $({ let arg: $arg = FromJs::from_js(ctx, args.get(tup!(@idx $arg)).cloned().unwrap())?; arg },)*
            //                         )
            //                     },
            //                 ),
            //             ),
            //         ).map_err(define_error::<BASE>("obj method", self.name))?;
            //         Ok(phase)
            //     }
            // }

        )*
    };
    (@one $($t:tt)*) => { 1 };
    (@idx A) => { 0 };
    (@idx B) => { 1 };
    (@idx C) => { 2 };
    (@idx D) => { 3 };
    (@idx E) => { 4 };
    (@idx F) => { 5 };
    (@idx G) => { 6 };
    (@idx H) => { 7 };
    (@idx I) => { 8 };
    (@idx J) => { 9 };
    (@idx K) => { 10 };
    (@idx L) => { 11 };
    (@idx M) => { 12 };
    (@idx N) => { 13 };
    (@idx O) => { 14 };
    (@idx P) => { 15 };
}

tup! {
    ,
    A,
    A B,
    A B C,
    A B C D,
    A B C D E,
    A B C D E F,
    A B C D E F G,
    A B C D E F G H,
    A B C D E F G H I,
    A B C D E F G H I J,
    A B C D E F G H I J K,
    A B C D E F G H I J K L,
    A B C D E F G H I J K L M,
    A B C D E F G H I J K L M N,
    A B C D E F G H I J K L M N O,
    A B C D E F G H I J K L M N O P,
}

pub fn tuple_match_size(actual: usize, expected: usize) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(Error::new_from_js_message(
            "array",
            "tuple",
            if actual < expected {
                "Not enough values"
            } else {
                "Too many values"
            },
        ))
    }
}
