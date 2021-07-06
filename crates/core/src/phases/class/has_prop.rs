use rquickjs::This;

use crate::inner::*;

pub struct HasProp<'a, FW, CLS> {
    pub(in crate) name: &'a str,
    pub(in crate) is_set: bool,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, CLS: Repr<FW>, FW> HasProp<'a, FW, CLS> {
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            is_set: false,
            framework: PhantomData,
        }
    }
}

impl<'a, FW, CLS, PROP, GET, SET> Implements<HasProp<'a, FW, CLS>>
    for Property<FW, CLS, PROP, GET, SET>
where
    FW: Framework,
    CLS: Type<FW>,
    ReprOf<FW, CLS>: Cast<CLS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build(self, phase: &mut HasProp<'a, FW, CLS>) -> Result<()> {
        if self.name == phase.name {
            phase.is_set = true;
        }
        Ok(())
    }
}

impl<'a, FW, CLS, PROP, GET> Implements<HasProp<'a, FW, CLS>> for ReadOnly<FW, CLS, PROP, GET>
where
    FW: Framework,
    CLS: Type<FW>,
    ReprOf<FW, CLS>: Cast<CLS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
{
    #[inline(always)]
    fn build(self, phase: &mut HasProp<'a, FW, CLS>) -> Result<()> {
        if self.name == phase.name {
            phase.is_set = true;
        }
        Ok(())
    }
}
