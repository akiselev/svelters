use crate::inner::*;

pub struct ReadOnly<CLS, PROP, GET> {
    pub(crate) name: &'static str,
    pub(crate) getter: GET,
    marker: PhantomData<(CLS, PROP)>,
}

impl<CLS, PROP, GET> ReadOnly<CLS, PROP, GET> {
    pub fn new(name: &'static str, getter: GET) -> Self {
        Self {
            name,
            getter,
            marker: PhantomData,
        }
    }
}
