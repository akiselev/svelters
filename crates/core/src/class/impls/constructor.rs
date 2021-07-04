use rquickjs::{Class, Rest};

use crate::inner::*;

pub struct ClassConstructor<FW, CLS, A, R, T>(
    pub(crate) T,
    pub(crate) PhantomData<(CLS, A, R, FW)>,
)
where
    FW: Container<CLS>;

unsafe impl<FW, T, A, R, CLS> Sync for ClassConstructor<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
    T: Constructor<FW, CLS, A, R>,
{
}

impl<FW, T, A, R, CLS> ClassConstructor<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
{
    pub(in crate) fn new(func: T) -> Self {
        ClassConstructor(func, PhantomData)
    }
}

impl<FW, T, A, R, CLS> TypeDef for ClassConstructor<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
    T: 'static + Constructor<FW, CLS, A, R>,
    R: 'static,
    A: 'static,
{
    fn load<'js>(_ctx: Ctx<'js>, exports: &Module<'js, Created>) -> Result<()> {
        exports.add(std::any::type_name::<CLS>())?;
        Ok(())
    }

    fn eval<'js>(
        &'static self,
        _ctx: Ctx<'js>,
        exports: &Module<'js, Loaded<Native>>,
    ) -> Result<()> {
        exports.set(
            std::any::type_name::<CLS>(),
            rquickjs::Func::new(
                std::any::type_name::<CLS>(),
                rquickjs::Class::<Obj<FW, CLS>>::constructor(
                    move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
                        let args = args.into_inner();
                        let res = self.0.try_constructor(ctx, &args)?;
                        Class::instance(ctx, res)
                    },
                ),
            ),
        )?;
        Ok(())
    }
}
