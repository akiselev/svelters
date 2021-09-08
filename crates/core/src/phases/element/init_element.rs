use rquickjs::Function;

use crate::{build_class, inner::*};

pub struct InitType<'a, BASE> {
    pub(in crate) element: &'a mut TypeDef,
    pub(in crate) marker: PhantomData<BASE>,
}

impl<'a, BASE> InitType<'a, BASE> {
    pub fn new(element: &'a mut TypeDef) -> Self {
        Self {
            element,
            marker: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, CLS, BASE, FUNC> Implements<InitType<'a, BASE>> for members::Event<CLS, FUNC>
where
    CLS: Type,
    BASE: Type + Cast<CLS>,
    FUNC: 'static + SubscribeFn,
{
    #[inline(always)]
    fn build_phase(self, phase: InitType<'a, BASE>) -> Result<InitType<'a, BASE>> {
        phase.element.event::<BASE, CLS, _>(self.name, self.func);
        Ok(phase)
    }
}
