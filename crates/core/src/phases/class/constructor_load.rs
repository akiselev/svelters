use crate::inner::*;

pub struct ConstructorLoad<'a, 'js, FW: Wraps<CLS>, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a Module<'js, Created>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, FW: Wraps<CLS>, CLS> ConstructorLoad<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a Module<'js, Created>) -> Self {
        Self {
            ctx,
            module,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, FW, CLS, R> Implements<ConstructorLoad<'a, 'b, FW, CLS>>
    for ClassConstructor<FW, CLS, (), R, T>
where
    FW: Framework + TypeDef<CLS> + Wraps<CLS>,
    Wrapped<FW, CLS>: for<'x> GetRef<'x, CLS>,
    T: Fn() -> R,
{
    fn build(self, input: &ConstructorLoad<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "-- Setting {}::{} in ConstructorLoad",
            std::any::type_name::<FW>(),
            std::any::type_name::<CLS>()
        );
        input.module.add(<FW as TypeDef<CLS>>::typename())?;
        Ok(())
    }
}
