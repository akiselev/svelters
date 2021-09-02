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

impl<'a, FUNC, THIS, CLS, RET> Implements<HasMember<'a, CLS>> for Method<CLS, (THIS,), RET, FUNC>
where
    CLS: Type,
    CLS: Cast<THIS>,
    THIS: 'static,
    RET: 'static + for<'js> IntoJs<'js>,
    FUNC: 'static + for<'x> Fn(&'x THIS) -> RET,
{
    #[inline(always)]
    fn build(self, phase: &mut HasMember<'a, CLS>) -> Result<()> {
        if self.name == phase.name {
            phase.is_set = true;
        }
        Ok(())
    }
}

impl<'a, CLS, PROP, GET, SET> Implements<HasMember<'a, CLS>> for Property<CLS, PROP, GET, SET>
where
    CLS: Type,
    CLS: Cast<CLS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build(self, phase: &mut HasMember<'a, CLS>) -> Result<()> {
        if self.name == phase.name {
            phase.is_set = true;
        }
        Ok(())
    }
}

impl<'a, CLS, PROP, GET> Implements<HasMember<'a, CLS>> for ReadOnly<CLS, PROP, GET>
where
    CLS: Type,
    CLS: Cast<CLS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
{
    #[inline(always)]
    fn build(self, phase: &mut HasMember<'a, CLS>) -> Result<()> {
        if self.name == phase.name {
            phase.is_set = true;
        }
        Ok(())
    }
}
