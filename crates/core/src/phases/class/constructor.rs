use std::{collections::HashMap, lazy::OnceCell};

use rquickjs::WithProto;

use crate::{build_class, inner::*};

pub struct ConstructorEval<'a, 'b, 'js, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) doc: &'b mut Document,
    pub(in crate) module: &'a QJsModule<'js, Loaded<Native>>,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, 'b, 'js, CLS> ConstructorEval<'a, 'b, 'js, CLS> {
    pub fn new(
        ctx: Ctx<'js>,
        module: &'a QJsModule<'js, Loaded<Native>>,
        doc: &'b mut Document,
    ) -> Self {
        Self {
            ctx,
            doc,
            module,
            framework: PhantomData,
        }
    }
}

pub struct ConstructorLoad<'a, 'js, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a QJsModule<'js, Created>,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, 'b, 'js, CLS> ConstructorLoad<'a, 'js, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a QJsModule<'js, Created>) -> Self {
        Self {
            ctx,
            module,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b, 'js: 'a + 'b, T, CLS> Implements<ConstructorEval<'a, 'b, 'js, CLS>>
    for members::Element<CLS, T>
where
    CLS: Type + Clone,
    T: 'static + Fn() -> Result<CLS>,
{
    #[inline(always)]
    fn build_phase(self, phase: &mut ConstructorEval<'a, 'b, 'js, CLS>) -> Result<()> {
        let members::Element { name, func, .. } = self;
        let proto = Object::new(phase.ctx)?;
        proto.set("__TYPE__", "Element")?;
        proto.set("__CLS__", CLS::CLASS_NAME)?;
        proto.set(
            "castAs",
            rquickjs::Func::new("castAs", rquickjs::Method(el_cast::<CLS>())),
        )?;
        build_class!(CLS with InitElementProto::<CLS>::new(phase.ctx, &proto));
        let proto_key = { phase.ctx.register(proto.into_value()) };

        let mut eltype = ElementType::new(name, move |ctx: Ctx| {
            let proto: Object = ctx
                .get_register(proto_key)
                .unwrap()
                .as_object()
                .unwrap()
                .clone();
            let res = (func)()?;
            let el = ElementNode::new::<CLS>(ctx, name, Obj::new(res))?;
            WithProto(el, proto).into_js(ctx)
        });
        build_class!(CLS with InitElement::<'_, CLS>::new(&mut eltype));
        phase.doc.inner.borrow_mut().types.insert(name, eltype);
        Ok(())
    }
}

#[inline(always)]
fn el_cast<CLS>() -> impl for<'js> Fn(&mut ElementNode<'js>, Ctx<'js>, String) -> Result<Value<'js>>
where
    CLS: Clone + Type,
{
    let cell = OnceCell::new();
    move |this: &mut ElementNode, ctx: Ctx, target: String| {
        tracing::info!("Casting Element to {}", CLS::CLASS_NAME);
        let map = cell.get_or_try_init(|| {
            let mut map: ElementCasterMap = HashMap::new();
            map.insert(
                CLS::CLASS_NAME,
                Box::new(move |ctx, this| {
                    let this = this.borrow();
                    let obj = Obj::<CLS>::from_js_ref(ctx, this.obj.clone().into_value())?;
                    obj.inner.with_cast(move |data: &CLS| {
                        let virt = Virt::<CLS>::new((*data).clone(), this.obj.clone());
                        let value = virt.into_js(ctx)?;
                        Result::<_>::Ok(value)
                    })
                }),
            );
            {
                build_class!(CLS with InitElementCaster::<'_, CLS>::new(&mut map));
            }
            Result::Ok(map)
        })?;

        match map.get(target.as_str()) {
            Some(func) => (*func)(ctx, this),
            None => Err(Error::new(
                ctx,
                format!("No cast function from {} to {}", CLS::CLASS_NAME, target),
            )),
        }
    }
}

macro_rules! tup {
    ($($($arg:ident)*,)*) => {
        $(
            impl<'a, 'b, 'js: 'a, T, CLS, $($arg,)*> Implements<ConstructorEval<'a, 'b, 'js, CLS>> for members::Constructor<CLS, ($($arg,)*), T>
            where
                CLS: Type,
                T: 'static + Fn($($arg,)*) -> Result<CLS>,
                $($arg: 'static + for<'x> FromJs<'x>,)*
            {
                #[inline(always)]
                fn build_phase(self, phase: &mut ConstructorEval<'a, 'b, 'js, CLS>) -> Result<()> {
                    JsClass::<Obj<CLS>>::register(phase.ctx)?;
                    JsClass::<Virt<CLS>>::register(phase.ctx)?;
                    phase.module.set(
                        <CLS as Type>::CLASS_NAME,
                        rquickjs::Func::new(
                            <CLS as Type>::CLASS_NAME,
                            JsClass::<Obj<CLS>>::constructor(move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
                                let args = args.into_inner();
                                let args_len = args.len();
                                tuple_match_size(args_len, {0 $(+ tup!(@one $arg))*})?;

                                let result = self.0(
                                    $({ let arg: $arg = FromJs::from_js(ctx, args.get(tup!(@idx $arg)).cloned().unwrap())?; arg },)*
                                )?;
                                JsClass::instance(ctx, Obj::<CLS>::new(result))
                            }),
                        ),
                    )?;
                    Ok(())
                }
            }

            impl<'a, 'js: 'a, T, CLS, R, $($arg,)*> Implements<ConstructorLoad<'a, 'js, CLS>> for members::Constructor<CLS, ($($arg,)*), T>
            where
                CLS: Type,
                T: Fn($($arg,)*) -> R,
            {
                #[inline(always)]
                fn build_phase(self, phase: &mut ConstructorLoad<'a, 'js, CLS>) -> Result<()> {
                    phase.module.add(<CLS as Type>::name())?;
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
