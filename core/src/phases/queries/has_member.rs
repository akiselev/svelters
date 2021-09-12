use crate::inner::*;

pub struct HasMember<'a, CLS> {
    pub(in crate) name: &'a str,
    pub(in crate) is_set: bool,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, CLS> HasMember<'a, CLS> {
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            is_set: false,
            framework: PhantomData,
        }
    }
}

impl<'a, FUNC, THIS, CLS, RET> Implements<HasMember<'a, CLS>>
    for members::Method<CLS, (THIS,), RET, FUNC>
where
    CLS: Type + Upcast<THIS>,
    THIS: 'static,
    RET: 'static + for<'js> IntoJs<'js>,
    FUNC: 'static + for<'x> Fn(&'x THIS) -> RET,
{
    #[inline(always)]
    fn build_phase(self, mut phase: HasMember<'a, CLS>) -> Result<HasMember<'a, CLS>> {
        if self.name == phase.name {
            phase.is_set = true;
        }
        Ok(phase)
    }
}

impl<'a, CLS, PROP, GET, SET> Implements<HasMember<'a, CLS>>
    for members::Property<CLS, PROP, GET, SET>
where
    CLS: Type + Upcast<CLS>,
    PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build_phase(self, mut phase: HasMember<'a, CLS>) -> Result<HasMember<'a, CLS>> {
        if self.name == phase.name {
            phase.is_set = true;
        }
        Ok(phase)
    }
}

impl<'a, CLS, PROP, GET> Implements<HasMember<'a, CLS>> for members::ReadOnly<CLS, PROP, GET>
where
    CLS: Type + Upcast<CLS>,
    PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
{
    #[inline(always)]
    fn build_phase(self, mut phase: HasMember<'a, CLS>) -> Result<HasMember<'a, CLS>> {
        if self.name == phase.name {
            phase.is_set = true;
        }
        Ok(phase)
    }
}
