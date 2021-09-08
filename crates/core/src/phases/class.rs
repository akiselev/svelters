use crate::{build_class, inner::*};

pub struct InitVirt<'a, 'js, BASE> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<BASE>,
}

impl<'a, 'js, BASE> InitVirt<'a, 'js, BASE> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

pub struct ConstructorEval<'a, 'b, 'js, CLS> {
    pub(in crate) rt: Rt,
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) doc: &'b mut Document,
    pub(in crate) module: &'a QJsModule<'js, Loaded<Native>>,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, 'b, 'js, CLS> ConstructorEval<'a, 'b, 'js, CLS> {
    pub fn new(
        rt: Rt,
        ctx: Ctx<'js>,
        module: &'a QJsModule<'js, Loaded<Native>>,
        doc: &'b mut Document,
    ) -> Self {
        Self {
            rt,
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

pub struct InitStatic<'a, 'js, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, 'js, CLS> InitStatic<'a, 'js, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

pub struct InitCaster<'a, CLS: Type> {
    pub(in crate) map: &'a mut ObjCasterMap<CLS>,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, CLS: Type> InitCaster<'a, CLS> {
    pub fn new(map: &'a mut ObjCasterMap<CLS>) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}

pub struct InitSuperCaster<'a, CLS: Type, SUPER> {
    pub(in crate) map: &'a mut ObjCasterMap<CLS>,
    pub(in crate) framework: PhantomData<(CLS, SUPER)>,
}

impl<'a, CLS: Type, SUPER> InitSuperCaster<'a, CLS, SUPER> {
    pub fn new(map: &'a mut ObjCasterMap<CLS>) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}
