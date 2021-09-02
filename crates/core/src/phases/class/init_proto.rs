use crate::{build_class, inner::*};

pub struct InitProto<'a, 'js, BASE> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<BASE>,
}

impl<'a, 'js, BASE> InitProto<'a, 'js, BASE> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, BASE, CLS, PROP, GET, SET> Implements<InitProto<'a, 'b, BASE>>
    for Property<CLS, PROP, GET, SET>
where
    CLS: Type,
    BASE: Type,
    BASE: Caster<CLS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitProto<'a, 'b, BASE>) -> Result<()> {
        let Property {
            name,
            getter,
            setter,
            ..
        } = self;

        let res = phase.obj.prop(
            name,
            Accessor::new(
                rquickjs::Method(move |this: JsClass<'b, Obj<BASE>>| {
                    let obj: &Obj<BASE> = this.as_ref();
                    obj.inner.with_cast(|data| {
                        let val = getter(data)?;
                        rquickjs::Result::Ok(val)
                    })
                }),
                rquickjs::Method(move |this: JsClass<'b, Obj<BASE>>, val: PROP| {
                    let obj: &Obj<BASE> = this.as_ref();
                    obj.inner.with_cast(|data| {
                        setter(data, val)?;
                        Result::<_>::Ok(())
                    })
                }),
            )
            .enumerable()
            .configurable(),
        );
        res
    }
}

impl<'a, 'b: 'a, BASE, CLS, PROP, GET> Implements<InitProto<'a, 'b, BASE>>
    for ReadOnly<CLS, PROP, GET>
where
    CLS: Type,
    BASE: Type,
    BASE: Caster<CLS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitProto<'a, 'b, BASE>) -> Result<()> {
        let ReadOnly { name, getter, .. } = self;

        // phase.obj(name).ok();
        let res = phase.obj.prop(
            name,
            Accessor::new_get(rquickjs::Method(move |this: JsClass<'b, Obj<BASE>>| {
                let obj: &Obj<BASE> = this.as_ref();
                obj.inner.with_cast(|data| {
                    let val = getter(data)?;
                    rquickjs::Result::Ok(val)
                })
            }))
            .enumerable(),
        );
        res
    }
}

impl<'a, 'b: 'a, CLS, SUPER> Implements<InitProto<'a, 'b, CLS>> for Superclass<SUPER>
where
    CLS: Type,
    SUPER: Type,
    CLS: Caster<SUPER>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitProto<'a, 'b, CLS>) -> Result<()> {
        build_class!(SUPER with InitProto::<'_, '_, CLS>::new(phase.ctx, phase.obj));
        Ok(())
    }
}

macro_rules! tup {
    ($($($arg:ident)*,)*) => {
        $(
        impl<'a, 'b: 'a, BASE, FUNC, THIS, CLS, RET, $($arg,)*> Implements<InitProto<'a, 'b, BASE>>
            for Method<CLS, (THIS, $($arg,)*), RET, FUNC>
        where
            CLS: Type,
            BASE: Type + Caster<THIS>,
            THIS: 'static,
            $($arg: 'static + for<'x> FromJs<'x>,)*
            RET: 'static + for<'js> IntoJs<'js>,
            FUNC: 'static + for<'x> Fn(&'x THIS, $($arg,)*) -> Result<RET>,
        {
            #[inline(always)]
            fn build(self, phase: &mut InitProto<'a, 'b, BASE>) -> Result<()> {
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
                                obj.inner.with_cast(|data| {
                                    (self.func)(
                                        data,
                                        $({ let arg: $arg = FromJs::from_js(ctx, args.get(tup!(@idx $arg)).cloned().unwrap())?; arg },)*
                                    )
                                })
                            },
                        ),
                    ),
                )?;
                Ok(())
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
