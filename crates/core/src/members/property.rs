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

impl<'a, 'b: 'a, BASE, CLS, PROP, GET, SET> Implements<InitProto<'a, 'b, BASE>>
    for members::Property<CLS, PROP, GET, SET>
where
    CLS: Type,
    BASE: Type + Cast<CLS>,
    PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build_phase(self, phase: InitProto<'a, 'b, BASE>) -> Result<InitProto<'a, 'b, BASE>> {
        let members::Property {
            name,
            getter,
            setter,
            ..
        } = self;

        let res = phase.obj.prop(
            name,
            Accessor::new(
                rquickjs::Method(move |this: JsClass<'b, Obj<BASE>>| {
                    let obj: &Obj<BASE> = this.as_ref();
                    let ptr = obj.borrow().unwrap();
                    let cls = ptr.cast();

                    let val = getter(cls.as_ref())?;
                    rquickjs::Result::Ok(val)
                }),
                rquickjs::Method(move |this: JsClass<'b, Obj<BASE>>, val: PROP| {
                    let obj: &Obj<BASE> = this.as_ref();
                    let ptr = obj.borrow().unwrap();
                    let cls = ptr.cast();

                    setter(cls.as_ref(), val)?;
                    Result::<_>::Ok(())
                }),
            )
            .enumerable()
            .configurable(),
        )?;
        Ok(phase)
    }
}

impl<'a, 'b: 'a, BASE, CLS, PROP, GET, SET> Implements<InitType<'a, BASE>>
    for members::Property<CLS, PROP, GET, SET>
where
    CLS: Type,
    BASE: Type + Cast<CLS>,
    PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build_phase(self, phase: InitType<'a, BASE>) -> Result<InitType<'a, BASE>> {
        let members::Property {
            name,
            getter,
            setter,
            ..
        } = self;

        phase
            .element
            .prop::<BASE, CLS, PROP, GET, SET>(name, getter, setter);
        Ok(phase)
    }
}

impl<'a, 'b, 'c, BASE, CLS, PROP, GET, SET> Implements<InitElementProto<'a, 'b, 'c, BASE>>
    for members::Property<CLS, PROP, GET, SET>
where
    CLS: Type,
    BASE: Type + Cast<CLS>,
    PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build_phase(
        self,
        phase: InitElementProto<'a, 'b, 'c, BASE>,
    ) -> Result<InitElementProto<'a, 'b, 'c, BASE>> {
        let members::Property {
            name,
            getter,
            setter,
            ..
        } = self;

        let res = phase.obj.prop(
            name,
            Accessor::new(
                rquickjs::Method(move |this: &mut ElementNode<'b>, ctx: Ctx<'b>| {
                    let obj = this.rt.get_instance::<BASE>(this.idx).unwrap();
                    let cls = obj.cast();

                    getter(cls.as_ref())
                }),
                rquickjs::Method(move |this: &mut ElementNode<'b>, ctx: Ctx<'b>, val: PROP| {
                    let obj = this.rt.get_instance::<BASE>(this.idx).unwrap();
                    let cls = obj.cast();
                    setter(cls.as_ref(), val)
                }),
            )
            .enumerable()
            .configurable(),
        )?;
        Ok(phase)
    }
}

impl<'a, 'b: 'a, BASE, CLS, PROP, GET, SET> Implements<InitVirt<'a, 'b, BASE>>
    for members::Property<CLS, PROP, GET, SET>
where
    CLS: Type,
    BASE: Type + Cast<CLS>,
    PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build_phase(self, phase: InitVirt<'a, 'b, BASE>) -> Result<InitVirt<'a, 'b, BASE>> {
        let members::Property {
            name,
            getter,
            setter,
            ..
        } = self;

        let res = phase.obj.prop(
            name,
            Accessor::new(
                rquickjs::Method(move |this: JsClass<'b, Virt<BASE>>| {
                    let obj: &Virt<BASE> = this.as_ref();
                    let base = obj.as_base::<BASE>().unwrap();
                    let ptr = base.cast();

                    let val = getter(ptr.as_ref())?;
                    rquickjs::Result::Ok(val)
                }),
                rquickjs::Method(move |this: JsClass<'b, Virt<BASE>>, val: PROP| {
                    let obj: &Virt<BASE> = this.as_ref();
                    let base = obj.as_base::<BASE>().unwrap();
                    let ptr = base.cast();

                    setter(ptr.as_ref(), val)?;
                    Result::<_>::Ok(())
                }),
            )
            .enumerable()
            .configurable(),
        )?;
        Ok(phase)
    }
}
