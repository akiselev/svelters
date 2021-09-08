use crate::inner::*;

pub struct Property<CLS, PROP, GET, SET> {
    pub(crate) name: &'static str,
    pub(crate) getter: GET,
    pub(crate) setter: SET,
    marker: PhantomData<(CLS, PROP)>,
}

impl<CLS, PROP, GET, SET> Property<CLS, PROP, GET, SET> {
    pub fn new(name: &'static str, getter: GET, setter: SET) -> Self {
        Self {
            name,
            getter,
            setter,
            marker: PhantomData,
        }
    }
}
