use crate::{build_class, inner::*};

pub struct ConstructorEval<'a, 'b, 'js, CLS> {
    pub(in crate) rt: Rt,
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) doc: &'b mut Document,
    pub(in crate) module: &'a QJsModule<'js, Loaded<Native>>,
    pub(in crate) marker: PhantomData<CLS>,
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
            marker: PhantomData,
        }
    }
}

pub struct ConstructorLoad<'a, 'js, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a QJsModule<'js, Created>,
    pub(in crate) marker: PhantomData<CLS>,
}

impl<'a, 'b, 'js, CLS> ConstructorLoad<'a, 'js, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a QJsModule<'js, Created>) -> Self {
        Self {
            ctx,
            module,
            marker: PhantomData,
        }
    }
}

pub struct InitProto<'a, 'js, BASE> {
    pub(in crate) depth: isize,
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) marker: PhantomData<BASE>,
}

impl<'a, 'js, BASE> InitProto<'a, 'js, BASE> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>, depth: isize) -> Self {
        Self {
            ctx,
            obj,
            depth,
            marker: PhantomData,
        }
    }
}

pub struct InitVirt<'a, 'js, BASE> {
    pub(in crate) depth: isize,
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) marker: PhantomData<BASE>,
}

impl<'a, 'js, BASE> InitVirt<'a, 'js, BASE> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>, depth: isize) -> Self {
        Self {
            ctx,
            obj,
            depth,
            marker: PhantomData,
        }
    }
}

pub struct InitImpl<'a, 'js, BASE> {
    pub(in crate) depth: isize,
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) marker: PhantomData<BASE>,
}

impl<'a, 'js, BASE> InitImpl<'a, 'js, BASE> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>, depth: isize) -> Self {
        Self {
            ctx,
            obj,
            depth,
            marker: PhantomData,
        }
    }
}

pub struct InitStatic<'a, 'js, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) marker: PhantomData<CLS>,
}

impl<'a, 'js, CLS> InitStatic<'a, 'js, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            marker: PhantomData,
        }
    }
}

pub(in crate) struct InitCaster<'a, CLS: Type> {
    pub(in crate) map: &'a mut CasterMap,
    pub(in crate) cls: Class<CLS>,
}

impl<'a, CLS: Type> InitCaster<'a, CLS> {
    pub fn new(map: &'a mut CasterMap) -> Self {
        Self {
            map,
            cls: Class::new(),
        }
    }
}

pub(in crate) struct InitSuperCaster<'a, CLS: Type, SUPER> {
    pub(in crate) map: &'a mut CasterMap,
    pub(in crate) cls: Class<CLS>,
    pub(in crate) sup: Class<SUPER>,
}

impl<'a, CLS: Type, SUPER> InitSuperCaster<'a, CLS, SUPER> {
    pub fn new(map: &'a mut CasterMap) -> Self {
        Self {
            map,
            cls: Class::new(),
            sup: Class::new(),
        }
    }
}

pub(in crate) fn define_error<CLS: Type>(
    _type: &'static str,
    name: &'static str,
) -> impl Fn(Error) -> Error {
    move |err| {
        exception!(
            "Error defining {} {}::{}: {}",
            _type,
            CLS::CLASS_NAME,
            name,
            err
        )
    }
}
