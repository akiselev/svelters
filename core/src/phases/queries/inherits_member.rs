use crate::{build_class, inner::*};

pub struct InheritsMember<'a, CLS> {
    pub(in crate) name: &'a str,
    pub(in crate) is_set: bool,
    pub(in crate) depth: isize,
    pub(in crate) max: isize,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, CLS> InheritsMember<'a, CLS> {
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

impl<'a, CLS, SUPER> Implements<InheritsMember<'a, CLS>> for members::Superclass<SUPER>
where
    CLS: Type + Upcast<CLS>,
    SUPER: Type,
{
    #[inline(always)]
    fn build_phase(self, mut phase: InheritsMember<'a, CLS>) -> Result<InheritsMember<'a, CLS>> {
        if build_class!(SUPER with HasMember::<'a, SUPER>::new(phase.name)).is_set {
            phase.is_set = true;
            return Ok(phase);
        }
        if phase.depth < phase.max || phase.max == -1 {
            let child = build_class!(SUPER with InheritsMember::<'a, SUPER>::inner(phase.name, phase.max, phase.depth + 1));
            if child.is_set {
                phase.is_set = true;
            }
        }
        Ok(phase)
    }
}
