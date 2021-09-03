use crate::inner::*;

pub trait IConstructor<'js> {
    // fn try_constructor<'js>(
    //     &'static self,
    //     ctx: Ctx<'js>,
    //     args: &Vec<Value<'js>>,
    // ) -> Result<Self::Return>;
}

// impl<FUNC, CLS, ARGS> IConstructor<CLS, ARGS> for FUNC
// where
//     CLS: Type,
//     FUNC: 'static + Fn<ARGS, Output = Result<CLS>>,
// {
//     const NUM_ARGS: usize = 0;

//     type Args = ();
//     type Return = Obj<CLS>;

//     fn try_constructor<'js>(
//         &'static self,
//         ctx: Ctx<'js>,
//         args: &Vec<Value<'js>>,
//     ) -> Result<Self::Return> {
//         let args_len = args.len();
//         tuple_match_size(args_len, 0)?;

//         let result = (*self)();
//         Ok(Obj::new(result?))
//     }
// }
