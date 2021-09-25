use crate::inner::*;

pub struct GtkProp<IMPL: ?Sized, GET, GRET, SET, SIN> {
    interface: &'static str,
    name: &'static str,
    getter: GET,
    setter: SET,
    marker: PhantomData<(GRET, SIN, IMPL)>,
}

impl<GET, GRET, SET, SIN> GtkProp<(), GET, GRET, SET, SIN> {
    #[inline(always)]
    pub fn new<IMPL>(
        interface: &'static str,
        name: &'static str,
        getter: GET,
        setter: SET,
    ) -> GtkProp<IMPL, GET, GRET, SET, SIN>
    where
        IMPL: 'static,
        GET: 'static + for<'js> Fn(Ctx<'js>, &IMPL) -> GRET,
        SET: 'static + for<'js> Fn(Ctx<'js>, &IMPL, SIN) -> Result<()>,
        SIN: 'static + for<'js> FromJs<'js>,
        GRET: 'static + for<'js> IntoJs<'js>,
    {
        GtkProp {
            name,
            getter,
            setter,
            interface,
            marker: PhantomData,
        }
    }
}

impl<IMPL, GET, GRET, SET, SIN> IProperty for GtkProp<IMPL, GET, GRET, SET, SIN>
where
    IMPL: 'static,
    GET: 'static + for<'js> Fn(Ctx<'js>, &IMPL) -> GRET,
    SET: 'static + for<'js> Fn(Ctx<'js>, &IMPL, SIN) -> Result<()>,
    SIN: 'static + for<'js> FromJs<'js>,
    GRET: 'static + for<'js> IntoJs<'js>,
{
    fn name<'a>(&'a self) -> Cow<'a, str> {
        self.name.into()
    }

    fn get<'js>(
        &self,
        ctx: Ctx<'js>,
        element: &dyn IElement,
    ) -> svelters_core::prelude::Result<Value<'js>> {
        let this: &dyn Upcast<IMPL> = element
            .query_ref()
            .ok_or_else(|| exception!("Element does not has an attribute named {}", self.name))?;
        let this = this.upcast();
        (self.getter)(ctx, &this).into_js(ctx)
    }

    fn set<'js>(
        &self,
        ctx: Ctx<'js>,
        element: &dyn IElement,
        value: Value<'js>,
    ) -> svelters_core::prelude::Result<()> {
        let this: &dyn Upcast<IMPL> = element
            .query_ref()
            .ok_or_else(|| exception!("Element does not implement the {} interface", self.name))?;
        let mut this = this.upcast();
        (self.setter)(ctx, &mut this, SIN::from_js(ctx, value)?)
    }
}
