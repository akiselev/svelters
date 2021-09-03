use crate::{build_class, inner::*};

pub struct InitElement<'a, BASE> {
    pub(in crate) element: &'a mut ElementType,
    pub(in crate) marker: PhantomData<BASE>,
}

impl<'a, BASE> InitElement<'a, BASE> {
    pub fn new(element: &'a mut ElementType) -> Self {
        Self {
            element,
            marker: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, BASE, CLS, PROP, GET, SET> Implements<InitElement<'a, BASE>>
    for members::Property<CLS, PROP, GET, SET>
where
    CLS: Type,
    BASE: Type + Caster<CLS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitElement<'a, BASE>) -> Result<()> {
        let members::Property {
            name,
            getter,
            setter,
            ..
        } = self;

        phase
            .element
            .prop::<BASE, CLS, PROP, GET, SET>(name, getter, setter);
        Ok(())
    }
}

impl<'a, 'b: 'a, BASE, CLS, PROP, GET> Implements<InitElement<'a, BASE>>
    for members::ReadOnly<CLS, PROP, GET>
where
    CLS: Type,
    BASE: Type + Caster<CLS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitElement<'a, BASE>) -> Result<()> {
        let members::ReadOnly { name, getter, .. } = self;

        phase.element.readonly::<BASE, CLS, PROP, GET>(name, getter);
        Ok(())
    }
}

impl<'a, 'b: 'a, CLS, SUPER> Implements<InitElement<'a, CLS>> for members::Superclass<SUPER>
where
    CLS: Type + Caster<SUPER>,
    SUPER: Type,
{
    #[inline(always)]
    fn build(self, phase: &mut InitElement<'a, CLS>) -> Result<()> {
        phase.element.inherits::<SUPER>();
        build_class!(SUPER with InitElement::<'_, CLS>::new(phase.element));
        Ok(())
    }
}
