use rquickjs::Function;

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

impl<'a, 'b: 'a, CLS, BASE, FUNC> Implements<InitElement<'a, BASE>> for members::Event<CLS, FUNC>
where
    CLS: Type,
    BASE: Type + Cast<CLS>,
    FUNC: 'static + SubscribeFn,
{
    #[inline(always)]
    fn build_phase(self, phase: InitElement<'a, BASE>) -> Result<InitElement<'a, BASE>> {
        phase.element.event::<BASE, CLS, _>(self.name, self.func);
        Ok(phase)
    }
}
