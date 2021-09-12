use crate::inner::*;

pub struct Method<CLS, ARGS, RET, FUNC> {
    pub(crate) name: &'static str,
    pub(crate) func: FUNC,
    marker: PhantomData<(CLS, ARGS, RET)>,
}

impl<FUNC, ARGS, RET, CLS> Method<CLS, ARGS, RET, FUNC> {
    pub(in crate) fn new(name: &'static str, func: FUNC) -> Self {
        Method {
            name,
            func,
            marker: PhantomData,
        }
    }
}

macro_rules! tup {
    ($($($arg:ident)*,)*) => {
        $(
            #[allow(unused_variables)]
            impl<'a, 'b: 'a, BASE, FUNC, CLS, RET, $($arg,)*> Implements<InitProto<'a, 'b, BASE>>
                for members::Method<CLS, (CLS, $($arg,)*), RET, FUNC>
            where
                CLS: Type,
                BASE: Type + Upcast<CLS>,
                $($arg: 'static + FromJs<'b>,)*
                RET: 'static + for<'js> IntoJs<'js>,
                FUNC: 'static + for<'x> Fn(&'x CLS, $($arg,)*) -> Result<RET>,
            {
                #[inline(always)]
                fn build_phase(self, phase: InitProto<'a, 'b, BASE>) -> Result<InitProto<'a, 'b, BASE>> {
                    phase.obj.set(
                        self.name,
                        rquickjs::Func::new(
                            self.name,
                            rquickjs::Method(
                                move |this: JsClass<'b, Obj<BASE>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                                    let args = args.into_inner();
                                    let args_len = args.len();
                                    tuple_match_size(args_len, {0 $(+ tup!(@one $arg))*})?;
                                    let obj: &Obj<BASE> = this.as_ref();
                                    let ptr = obj.borrow().unwrap();
                                    (self.func)(
                                        &ptr.upcast(),
                                        $({ let arg: $arg = FromJs::from_js(ctx, args.get(tup!(@idx $arg)).cloned().unwrap())?; arg },)*
                                    )
                                },
                            ),
                        ),
                    ).map_err(define_error::<BASE>("obj method", self.name))?;
                    Ok(phase)
                }
            }

            impl<'a, 'b, 'c, BASE, FUNC, CLS, RET, $($arg,)*> Implements<InitElementProto<'a, 'b, 'c, BASE>>
                for members::Method<CLS, (CLS, $($arg,)*), RET, FUNC>
            where
                CLS: Type,
                BASE: Type + Upcast<CLS>,
                RET: 'static + IntoJs<'b>,
                FUNC: 'static + for<'x> Fn(&'x CLS, $($arg,)*) -> Result<RET>,
                $($arg: 'static + for<'x> FromJs<'x>,)*
            {
                #[inline(always)]
                fn build_phase(self, phase: InitElementProto<'a, 'b, 'c, BASE>) -> Result<InitElementProto<'a, 'b, 'c, BASE>> {
                    phase.obj.set(
                        self.name,
                        rquickjs::Func::new(
                            self.name,
                            rquickjs::Method(
                                move |this: &mut ElementNode<'b>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                                    let args = args.into_inner();
                                    let args_len = args.len();
                                    tuple_match_size(args_len, {0 $(+ tup!(@one $arg))*})?;
                                    let obj = this.rt.get_instance::<BASE>(this.idx).unwrap();
                                    let cls = obj.upcast();
                                    let res = (self.func)(
                                        &cls,
                                        $({ let arg: $arg = FromJs::from_js(ctx, args.get(tup!(@idx $arg)).cloned().unwrap())?; arg },)*
                                    )?;
                                    res.into_js(ctx)
                                },
                            ),
                        ),
                    ).map_err(define_error::<BASE>("el method", self.name))?;
                    Ok(phase)
                }
            }

            impl<'a, 'b: 'a, BASE, FUNC, CLS, RET, $($arg,)*> Implements<InitVirt<'a, 'b, BASE>>
                for members::Method<CLS, (CLS, $($arg,)*), RET, FUNC>
            where
                CLS: Type,
                BASE: Type + Upcast<CLS>,
                RET: 'static +  IntoJs<'b>,
                FUNC: 'static + for<'x> Fn(&'x CLS, $($arg,)*) -> Result<RET>,
                $($arg: 'static + for<'x> FromJs<'x>,)*
            {
                #[inline(always)]
                fn build_phase(self, phase: InitVirt<'a, 'b, BASE>) -> Result<InitVirt<'a, 'b, BASE>> {
                    phase.obj.set(
                        self.name,
                        rquickjs::Func::new(
                            self.name,
                            rquickjs::Method(
                                move |this: JsClass<'b, Virt<BASE>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                                    let args = args.into_inner();
                                    let args_len = args.len();
                                    tuple_match_size(args_len, {0 $(+ tup!(@one $arg))*})?;
                                    let obj: &Virt<BASE> = this.as_ref();
                                    let ptr = obj.cls.upcast();

                                    let res = (self.func)(
                                        &ptr,
                                        $({ let arg: $arg = FromJs::from_js(ctx, args.get(tup!(@idx $arg)).cloned().unwrap())?; arg },)*
                                    )?;
                                    Result::<_>::Ok(res.into_js(ctx))
                                },
                            ),
                        ),
                    ).map_err(define_error::<BASE>("virt method", self.name))?;
                    Ok(phase)
                }
            }

            impl<'a, 'b: 'a, BASE, FUNC, CLS, RET, $($arg,)*> Implements<InitImpl<'a, 'b, BASE>>
                for members::Method<CLS, (CLS, $($arg,)*), RET, FUNC>
            where
                CLS: Type,
                BASE: Type + Upcast<CLS>,
                RET: 'static +  IntoJs<'b>,
                FUNC: 'static + for<'x> Fn(&'x CLS, $($arg,)*) -> Result<RET>,
                $($arg: 'static + for<'x> FromJs<'x>,)*
            {
                #[inline(always)]
                fn build_phase(self, phase: InitImpl<'a, 'b, BASE>) -> Result<InitImpl<'a, 'b, BASE>> {
                    phase.obj.set(
                        self.name,
                        rquickjs::Func::new(
                            self.name,
                            rquickjs::Method(
                                move |this: JsClass<'b, Impl<BASE>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                                    let args = args.into_inner();
                                    let args_len = args.len();
                                    tuple_match_size(args_len, {0 $(+ tup!(@one $arg))*})?;
                                    let obj: &Impl<BASE> = this.as_ref();
                                    let ptr = obj.cls.upcast();

                                    let res = (self.func)(
                                        &ptr,
                                        $({ let arg: $arg = FromJs::from_js(ctx, args.get(tup!(@idx $arg)).cloned().unwrap())?; arg },)*
                                    )?;
                                    Result::<_>::Ok(res.into_js(ctx))
                                },
                            ),
                        ),
                    ).map_err(define_error::<BASE>("impl method", self.name))?;
                    Ok(phase)
                }
            }
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
