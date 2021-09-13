use crate::inner::*;

impl<'a, 'b, 'js: 'a + 'b, T, CLS> Implements<ConstructorEval<'a, 'b, 'js, CLS>>
    for members::Element<CLS, T>
where
    CLS: Type,
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
        build!(CLS::InitElementProto::<CLS>[phase.ctx, &proto, 0]);
        initialize_element_node_proto::<CLS>(&proto)?;

        let proto_key = { phase.ctx.register(proto.into_value()) };
        let rt = phase.rt.weak();
        {
            let mut inner = phase.rt.metadata.deref().borrow_mut();
            inner.elements.insert(
                self.name,
                ElementDef::new::<CLS, _>(
                    name,
                    proto_key,
                    constructor_fn(rt, name, func, proto_key),
                ),
            );
            tracing::info!("Keys: {:?}", inner.elements.keys());
        }
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
    CLS: Type,
    T: 'static + Fn() -> Result<CLS>,
{
    move |ctx| {
        let proto: Object = ctx
            .get_register(proto_key)
            .ok_or_else(|| exception!("Could not get proto for type {}", CLS::CLASS_NAME))?
            .as_object()
            .ok_or_else(|| exception!("Proto for type {} is not an object", CLS::CLASS_NAME))?
            .clone();
        let res = (func)()?;
        let el = ElementNode::new::<CLS>(rt.upgrade().unwrap(), ctx, name, res)?;
        WithProto(el, proto).into_js(ctx)
    }
}

impl<'a, 'b, 'js: 'a + 'b, T, CLS> Implements<ConstructorEval<'a, 'b, 'js, CLS>>
    for members::ElementCtx<CLS, T>
where
    CLS: Type,
    T: 'static + for<'js_> Fn(Ctx<'js_>) -> Result<CLS>,
{
    #[inline(always)]
    fn build_phase(
        self,
        phase: ConstructorEval<'a, 'b, 'js, CLS>,
    ) -> Result<ConstructorEval<'a, 'b, 'js, CLS>> {
        let members::ElementCtx { name, element, .. } = self;
        tracing::info!("ElementCtx constructor for {}", name);
        let proto = Object::new(phase.ctx)?;
        proto.set("__TYPE__", "Element")?;
        proto.set("__CLS__", CLS::CLASS_NAME)?;
        build!(CLS::InitElementProto::<CLS>[phase.ctx, &proto, 0]);
        initialize_element_node_proto::<CLS>(&proto)?;

        let proto_key = { phase.ctx.register(proto.into_value()) };
        let rt = phase.rt.weak();
        {
            let mut inner = phase.rt.metadata.deref().borrow_mut();
            inner.elements.insert(
                self.name,
                ElementDef::new::<CLS, _>(
                    name,
                    proto_key,
                    constructor_ext(rt, name, element, proto_key),
                ),
            );
            tracing::info!("Keys: {:?}", inner.elements.keys());
        }
        Ok(phase)
    }
}

fn constructor_ext<'js, T, CLS>(
    rt: WeakRt,
    name: &'static str,
    func: T,
    proto_key: RegisteryKey,
) -> impl for<'x> Fn(Ctx<'x>) -> Result<Value<'x>>
where
    CLS: Type,
    T: 'static + for<'js_> Fn(Ctx<'js_>) -> Result<CLS>,
{
    move |ctx| {
        let proto: Object = ctx
            .get_register(proto_key)
            .ok_or_else(|| exception!("Could not get proto for type {}", CLS::CLASS_NAME))?
            .as_object()
            .ok_or_else(|| exception!("Proto for type {} is not an object", CLS::CLASS_NAME))?
            .clone();
        tracing::info!("Constructing element {}", name);
        let res = (func)(ctx)?;
        let el = ElementNode::new::<CLS>(rt.upgrade().unwrap(), ctx, name, res)?;
        WithProto(el, proto).into_js(ctx)
    }
}
