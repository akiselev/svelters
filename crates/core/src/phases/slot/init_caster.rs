use std::collections::HashMap;

use rquickjs::This;

use crate::{build_class, inner::*};

pub struct InitCaster<'a, 'js, FW, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, CLS: Type<FW>, FW: 'static> InitCaster<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'js, FW, CLS> Implements<InitCaster<'a, 'js, FW, CLS>> for JsClassLoader<FW, CLS>
where
    FW: Framework,
    CLS: Type<FW>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitCaster<FW, CLS>) -> Result<()> {
        println!("Calling InitCaster for {}", CLS::CLASS_NAME);
        // build_class!(FW::CLS with InitCaster::<FW, CLS>::new(ctx, _static));

        Ok(())
    }
}


impl<'a, 'js, FW, CLS, SUPER> Implements<InitCaster<'a, 'js, FW, CLS>> for Superclass<FW, CLS, SUPER>
where
    FW: Framework,
    CLS: Type<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, CLS>: Cast<SUPER>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitCaster<FW, CLS>) -> Result<()> {
        println!("Calling SUPERINITCASTER for {} of {}", CLS::CLASS_NAME, SUPER::CLASS_NAME);
        
        Ok(())
    }
}
