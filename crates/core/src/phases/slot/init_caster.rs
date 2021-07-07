use std::collections::HashMap;

use rquickjs::This;

use crate::{build_phase, inner::*};

pub struct InitCaster<FW, CLS> {
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'js, CLS: Type<FW>, FW: 'static> InitCaster<FW, CLS> {
    pub fn new() -> Self {
        Self {
            framework: PhantomData,
        }
    }
}

// impl<FW, CLS, SUPER> Implements<InitCaster<FW, CLS>> for Superclass<FW, CLS, SUPER>
// where
//     FW: Framework,
//     CLS: Type<FW>,
//     SUPER: Type<FW>,
//     ReprOf<FW, CLS>: Cast<SUPER>,
// {
//     #[inline(always)]
//     fn build(self, phase: &mut InitCaster<FW, CLS>) -> Result<()> {
//         build_phase!(FW::SUPER with InitSuperProto::<'_, '_, FW, CLS, SUPER>::new(phase.ctx, phase.obj));
//         Ok(())
//     }
// }

impl<'c, FW, CLS, PHASE> Implements<InitCaster<FW, CLS>> for TypeBuilder<'c, FW, CLS, PHASE>
where
    FW: Framework,
    CLS: Type<FW>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitCaster<FW, CLS>) -> Result<()> {
        unsafe { CLS::metadata().init()? };
        Ok(())
    }
}
