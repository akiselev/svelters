use rquickjs::This;

use crate::{build_class, inner::*};

pub struct InheritsMember<'a, FW, CLS> {
    pub(in crate) name: &'a str,
    pub(in crate) is_set: bool,
    pub(in crate) depth: isize,
    pub(in crate) max: isize,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, CLS: Repr<FW>, FW> InheritsMember<'a, FW, CLS> {
    pub fn new(name: &'a str, max: isize) -> Self {
        Self {
            name,
            is_set: false,
            depth: 0,
            max,
            framework: PhantomData,
        }
    }

    fn inner(name: &'a str, max: isize, depth: isize) -> Self {
        Self {
            name,
            is_set: false,
            depth,
            max,
            framework: PhantomData,
        }
    }
}

impl<'a, FW, CLS, SUPER> Implements<InheritsMember<'a, FW, CLS>> for Superclass<FW, CLS, SUPER>
where
    FW: Framework,
    CLS: Type<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, CLS>: Cast<CLS>,
{
    #[inline(always)]
    fn build(self, phase: &mut InheritsMember<'a, FW, CLS>) -> Result<()> {
        if build_class!(FW::SUPER with HasMember::<'a, FW, SUPER>::new(phase.name)).is_set {
            phase.is_set = true;
            return Ok(());
        }
        if phase.depth < phase.max || phase.max == -1 {
            let child = build_class!(FW::SUPER with InheritsMember::<'a, FW, SUPER>::inner(phase.name, phase.max, phase.depth + 1));
            if child.is_set {
                phase.is_set = true;
            }
        }
        Ok(())
    }
}
