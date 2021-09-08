use crate::inner::*;

impl<'a, 'b, 'js: 'a + 'b, T, CLS> Implements<ConstructorEval<'a, 'b, 'js, CLS>>
    for members::Element<CLS, T>
where
    CLS: Type + Clone,
    T: 'static + Fn() -> Result<CLS>,
{
    #[inline(always)]
    fn build_phase(
        self,
        phase: ConstructorEval<'a, 'b, 'js, CLS>,
    ) -> Result<ConstructorEval<'a, 'b, 'js, CLS>> {
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

        let rt = phase.rt.weak();
        let mut eltype = ElementType::new(name, move |ctx: Ctx| {
            let proto: Object = ctx
                .get_register(proto_key)
                .unwrap()
                .as_object()
                .unwrap()
                .clone();
            let res = (func)()?;
            let el = ElementNode::new::<CLS>(rt.upgrade().unwrap(), ctx, name, res)?;
            WithProto(el, proto).into_js(ctx)
        });
        build_class!(CLS with InitElement::<'_, CLS>::new(&mut eltype));
        phase.doc.inner.borrow_mut().types.insert(name, eltype);
        Ok(phase)
    }
}

#[inline(always)]
fn el_cast<CLS>() -> impl for<'js> Fn(&mut ElementNode<'js>, Ctx<'js>, String) -> Result<Value<'js>>
where
    CLS: Clone + Type,
{
    // let cell = OnceCell::new();
    move |this: &mut ElementNode, ctx: Ctx, target: String| {
        tracing::info!("Casting Element to {}", CLS::CLASS_NAME);
        // TODO: FIX CASTER
        // let map = cell.get_or_try_init(|| {
        //     let mut map: ElementCasterMap = HashMap::new();
        //     map.insert(
        //         CLS::CLASS_NAME,
        //         Box::new(move |ctx, this| {
        //             let this = this.borrow();
        //             let obj = Obj::<CLS>::from_js_ref(ctx, this.obj.clone().into_value())?;
        //             let ptr = obj.borrow().unwrap();
        //             let virt = Virt::<CLS>::new(obj.rt.clone(), obj.idx, this.obj.clone());
        //             let value = virt.into_js(ctx)?;
        //             Result::Ok(value)
        //         }),
        //     );
        //     {
        //         build_class!(CLS with InitElementCaster::<'_, CLS>::new(&mut map));
        //     }
        //     Result::<_>::Ok(map)
        // })?;

        // match map.get(target.as_str()) {
        //     Some(func) => (*func)(ctx, this),
        //     None => Err(Error::new(
        //         ctx,
        //         format!("No cast function from {} to {}", CLS::CLASS_NAME, target),
        //     )),
        // }
        Ok(Value::new_undefined(ctx))
    }
}
