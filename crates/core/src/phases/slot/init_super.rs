use crate::{build_class, inner::*};

pub struct InitSuperCaster<'a, CLS: Type, SUPER> {
    pub(in crate) map: &'a mut ObjCasterMap<CLS>,
    pub(in crate) framework: PhantomData<(CLS, SUPER)>,
}

impl<'a, CLS: Type, SUPER> InitSuperCaster<'a, CLS, SUPER> {
    pub fn new(map: &'a mut ObjCasterMap<CLS>) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, CLS, SUPER, NESTED> Implements<InitSuperCaster<'a, CLS, SUPER>>
    for members::Superclass<NESTED>
where
    CLS: Type,
    SUPER: Type,
    NESTED: Clone + Type,
    CLS: Cast<NESTED>,
{
    #[inline(always)]
    fn build_phase(
        self,
        phase: InitSuperCaster<'a, CLS, SUPER>,
    ) -> Result<InitSuperCaster<'a, CLS, SUPER>> {
        phase.map.insert(
            NESTED::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj: &Obj<CLS> = this.as_ref();
                let this = this.clone();
                let ptr = obj.borrow().unwrap();
                Virt::<NESTED>::new(obj.rt.clone(), obj.idx, this).into_js(ctx)
            }),
        );

        build_class!(NESTED with InitSuperCaster::<'_, CLS, NESTED>::new(phase.map));
        Ok(phase)
    }
}
