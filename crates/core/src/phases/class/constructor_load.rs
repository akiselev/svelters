use crate::inner::*;

pub struct ConstructorLoad<'a, 'js, FW, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a QJsModule<'js, Created>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, CLS: Repr<FW>, FW> ConstructorLoad<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a QJsModule<'js, Created>) -> Self {
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
    FW: Framework,
    CLS: TypeDef<FW>,
    T: Fn() -> R,
{
    fn build(self, input: &ConstructorLoad<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "-- Setting {}::{} in ConstructorLoad",
            std::any::type_name::<FW>(),
            std::any::type_name::<CLS>()
        );
        input.module.add(<CLS as TypeDef<FW>>::typename())?;
        Ok(())
    }
}
