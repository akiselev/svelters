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
        build_class!(CLS with InitElementProto::<CLS>::new(phase.ctx, &proto));

        let proto_key = { phase.ctx.register(proto.into_value()) };

        let rt = phase.rt.weak();
        phase.rt.inner.deref().borrow_mut().elements.insert(
            self.name,
            smallbox!(constructor_fn(rt, name, func, proto_key)),
        );
        Ok(phase)
    }
}

fn constructor_fn<'js, T, CLS>(
    rt: WeakRt,
    name: &'static str,
    func: T,
    proto_key: RegisteryKey,
) -> impl for<'x> Fn(Ctx<'x>) -> Result<Value<'x>>
where
    CLS: Type + Clone,
    T: 'static + Fn() -> Result<CLS>,
{
    move |ctx| {
        let proto: Object = ctx
            .get_register(proto_key)
            .unwrap()
            .as_object()
            .unwrap()
            .clone();
        let res = (func)()?;
        let el = ElementNode::new::<CLS>(rt.upgrade().unwrap(), ctx, name, res)?;
        WithProto(el, proto).into_js(ctx)
    }
}
