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

impl<'a, 'b: 'a, BASE, CLS, PROP, GET> Implements<InitProto<'a, 'b, BASE>>
    for members::ReadOnly<CLS, PROP, GET>
where
    CLS: Type,
    BASE: Type + Cast<CLS>,
    PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
{
    #[inline(always)]
    fn build_phase(self, phase: InitProto<'a, 'b, BASE>) -> Result<InitProto<'a, 'b, BASE>> {
        let members::ReadOnly { name, getter, .. } = self;

        // phase.obj(name).ok();
        let res = phase.obj.prop(
            name,
            Accessor::new_get(rquickjs::Method(move |this: JsClass<'b, Obj<BASE>>| {
                let obj: &Obj<BASE> = this.as_ref();
                let ptr = obj.borrow().unwrap();
                let cls = ptr.cast();
                let val = getter(cls.as_ref())?;
                rquickjs::Result::Ok(val)
            }))
            .enumerable(),
        )?;
        Ok(phase)
    }
}

impl<'a, 'b: 'a, BASE, CLS, PROP, GET> Implements<InitType<'a, BASE>>
    for members::ReadOnly<CLS, PROP, GET>
where
    CLS: Type,
    BASE: Type + Cast<CLS>,
    PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
{
    #[inline(always)]
    fn build_phase(self, phase: InitType<'a, BASE>) -> Result<InitType<'a, BASE>> {
        let members::ReadOnly { name, getter, .. } = self;

        phase.element.readonly::<BASE, CLS, PROP, GET>(name, getter);
        Ok(phase)
    }
}

impl<'a, 'b, 'c, BASE, CLS, PROP, GET> Implements<InitElementProto<'a, 'b, 'c, BASE>>
    for members::ReadOnly<CLS, PROP, GET>
where
    CLS: Type,
    BASE: Type + Cast<CLS>,
    PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
{
    #[inline(always)]
    fn build_phase(
        self,
        phase: InitElementProto<'a, 'b, 'c, BASE>,
    ) -> Result<InitElementProto<'a, 'b, 'c, BASE>> {
        let members::ReadOnly { name, getter, .. } = self;

        let res = phase.obj.prop(
            name,
            Accessor::new_get(rquickjs::Method(
                move |this: &mut ElementNode<'b>, ctx: Ctx<'b>| {
                    let obj = this.rt.get_instance::<BASE>(this.idx).unwrap();
                    let cls = obj.cast();
                    let val = getter(cls.as_ref())?;
                    rquickjs::Result::Ok(val)
                },
            ))
            .enumerable(),
        )?;
        Ok(phase)
    }
}

impl<'a, 'b: 'a, BASE, CLS, PROP, GET> Implements<InitVirt<'a, 'b, BASE>>
    for members::ReadOnly<CLS, PROP, GET>
where
    CLS: Type,
    BASE: Type + Cast<CLS>,
    PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
{
    #[inline(always)]
    fn build_phase(self, phase: InitVirt<'a, 'b, BASE>) -> Result<InitVirt<'a, 'b, BASE>> {
        let members::ReadOnly { name, getter, .. } = self;

        let res = phase.obj.prop(
            name,
            Accessor::new_get(rquickjs::Method(move |this: JsClass<'b, Virt<BASE>>| {
                let obj: &Virt<BASE> = this.as_ref();
                let base = obj.as_base::<BASE>().unwrap();
                let ptr = base.cast();

                let val = getter(ptr.as_ref())?;
                rquickjs::Result::Ok(val)
            }))
            .enumerable(),
        )?;
        Ok(phase)
    }
}
