use std::marker::PhantomData;

use rquickjs::{qjs::c_char, Ctx, InputAccessor, IntoJs, Value};

use super::marshall::*;
use super::primitives::*;
use crate::common::*;

pub fn invoke_extern_0a<'i, 'js, MR>(
    mut input: InputAccessor<'i, 'js>,
    func: unsafe extern "C" fn() -> <MR as MarshallResult<'js>>::Type,
) -> Result<Value<'js>>
where
    MR: InitMarshallResult<'i, 'js> + MarshallResult<'js>,
{
    let res_marshall = MR::init_marshall_result(&mut input)?;
    let res = unsafe { func() };
    let res = res_marshall.marshall_result(res)?;
    res.into_js(input.ctx())
}

macro arg {
    ($ma:ident[$i:lifetime, $js:lifetime] => $input:ident[$num:literal]) => {{
        {<$ma as InitMarshallArg<$i, $js, $num>>::init_marshall_arg(&mut $input)}
    }},
    ($arg:ident) => {
        if $arg.use_ref() {
            $arg.marshall_as_ref()
        } else {
            $arg.marshall_as_owned()
        }
    }
}

pub fn invoke_extern_1a<'i, 'js, MA, MR>(
    mut input: InputAccessor<'i, 'js>,
    func: unsafe extern "C" fn(
        <<MA as InitMarshallArg<'i, 'js, 0>>::Output as MarshallArgument<'js>>::Type,
    ) -> <MR as MarshallResult<'js>>::Type,
) -> Result<Value<'js>>
where
    MA: InitMarshallArg<'i, 'js, 0>,
    <MA as InitMarshallArg<'i, 'js, 0>>::Output: MarshallArgument<'js>,
    MR: InitMarshallResult<'i, 'js> + MarshallResult<'js>,
{
    let arg1 = arg!(MA['i, 'js] => input[0])?;
    let arg1 = arg1.marshall_arg()?;
    let res = unsafe { func(arg!(arg1)?) };
    let mr = <MR as InitMarshallResult<'i, 'js>>::init_marshall_result(&mut input)?;
    let res_marshall = mr.marshall_result(res)?;
    res_marshall.into_js(input.ctx())
}

pub fn invoke_extern_2a<'i, 'js, MA, MR>(
    mut input: InputAccessor<'i, 'js>,
    func: unsafe extern "C" fn(
        <<MA as InitMarshallArg<'i, 'js, 0>>::Output as MarshallArgument<'js>>::Type,
        <<MA as InitMarshallArg<'i, 'js, 1>>::Output as MarshallArgument<'js>>::Type,
    ) -> <MR as MarshallResult<'js>>::Type,
) -> Result<Value<'js>>
where
    MA: InitMarshallArg<'i, 'js, 0>,
    <MA as InitMarshallArg<'i, 'js, 0>>::Output: MarshallArgument<'js>,
    MA: InitMarshallArg<'i, 'js, 1>,
    <MA as InitMarshallArg<'i, 'js, 1>>::Output: MarshallArgument<'js>,
    MR: InitMarshallResult<'i, 'js> + MarshallResult<'js>,
{
    let arg1 = arg!(MA['i, 'js] => input[0])?;
    let arg1 = arg1.marshall_arg()?;
    let arg2 = arg!(MA['i, 'js] => input[1])?;
    let arg2 = arg2.marshall_arg()?;

    let res = unsafe { func(arg!(arg1)?, arg!(arg2)?) };
    let mr = <MR as InitMarshallResult<'i, 'js>>::init_marshall_result(&mut input)?;
    let res_marshall = mr.marshall_result(res)?;
    res_marshall.into_js(input.ctx())
}

pub fn test_g<'i, 'js>(input: InputAccessor<'i, 'js>) {
    type Marsh = ();
    type MarshRet = ();
    invoke_extern_1a::<
        'i,
        'js,
        (SelfMarshall<gtk4::Button, gtk4::ffi::GtkButton, None>,),
        DefaultMarshall<*const c_char, String>,
    >(input, gtk4::ffi::gtk_button_get_label)
    .unwrap();
}
