use rquickjs::This;

use crate::{build_phase, inner::*};

pub struct InheritsProp<'a, FW, CLS> {
    pub(in crate) name: &'a str,
    pub(in crate) is_set: bool,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, CLS: Repr<FW>, FW> InheritsProp<'a, FW, CLS> {
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            is_set: false,
            framework: PhantomData,
        }
    }
}

impl<'a, FW, CLS, SUPER> Implements<InheritsProp<'a, FW, CLS>> for Superclass<FW, CLS, SUPER>
where
    FW: Framework,
    CLS: Type<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, CLS>: Cast<CLS>,
{
    #[inline(always)]
    fn build(self, phase: &mut InheritsProp<'a, FW, CLS>) -> Result<()> {
        if build_phase!(FW::CLS with HasProp::<'a, FW, SUPER>::new(phase.name)).is_set {
            phase.is_set = true;
        }
        Ok(())
    }
}

impl<'a, FW, CLS, PROP, GET, SET, SUPER> Implements<InheritsProp<'a, FW, CLS>>
    for Property<FW, SUPER, PROP, GET, SET>
where
    FW: Framework,
    CLS: Type<FW>,
    ReprOf<FW, CLS>: Cast<CLS>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
{
    #[inline(always)]
    fn build(self, phase: &mut InheritsProp<'a, FW, CLS>) -> Result<()> {
        if self.name == phase.name {
            phase.is_set = true;
        }
        Ok(())
    }
}

impl<'a, FW, CLS, PROP, GET, SUPER> Implements<InheritsProp<'a, FW, CLS>>
    for ReadOnly<FW, SUPER, PROP, GET>
where
    FW: Framework,
    CLS: Type<FW>,
    ReprOf<FW, CLS>: Cast<CLS>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
{
    #[inline(always)]
    fn build(self, phase: &mut InheritsProp<'a, FW, CLS>) -> Result<()> {
        if self.name == phase.name {
            phase.is_set = true;
        }
        Ok(())
    }
}
