use std::marker::PhantomData;

use rquickjs::{qjs::c_char, Ctx, InputAccessor, IntoJs, Value};

use super::default::*;
use super::marshall::*;
use super::primitives::*;
use crate::common::*;

type MRS<'i, 'js, T> = <T as InitMarshall<'i, 'js>>::Output;

pub fn invoke_extern_0a<'i, 'js, M>(
    mut input: InputAccessor<'i, 'js>,
    func: unsafe extern "C" fn() -> <MRS<'i, 'js, M> as MarshallResult<'js, MRS<'i, 'js, M>>>::Type,
) -> Result<Value<'js>>
where
    M: InitMarshall<'i, 'js>,
    MRS<'i, 'js, M>: MarshallResult<'js, MRS<'i, 'js, M>>,
{
    let mut state = M::init_marshall_state(&mut input)?;
    let res = unsafe { func() };
    let mr = <MRS<'i, 'js, M> as MarshallResult<'js, MRS<'i, 'js, M>>>::marshall_result(
        &mut state, res,
    )?;
    mr.into_js(input.ctx())
}

macro arg($arg:ident) {
    if $arg.use_ref() {
        $arg.marshall_as_ref()
    } else {
        $arg.marshall_as_owned()
    }
}

pub fn invoke_extern_1a<'i, 'js, M>(
    mut input: InputAccessor<'i, 'js>,
    func: unsafe extern "C" fn(
        <MRS<'i, 'js, M> as MarshallArgument<'js, MRS<'i, 'js, M>, 1>>::Type,
    )
        -> <MRS<'i, 'js, M> as MarshallResult<'js, MRS<'i, 'js, M>>>::Type,
) -> Result<Value<'js>>
where
    M: InitMarshall<'i, 'js>,
    MRS<'i, 'js, M>: MarshallResult<'js, MRS<'i, 'js, M>>,
    MRS<'i, 'js, M>: MarshallArgument<'js, MRS<'i, 'js, M>, 1>,
{
    let mut state = M::init_marshall_state(&mut input)?;
    let arg1 =
        <MRS<'i, 'js, M> as MarshallArgument<'js, MRS<'i, 'js, M>, 1>>::marshall_arg(&mut state)?;
    let res = unsafe { func(arg!(arg1)?) };
    let mr = <MRS<'i, 'js, M> as MarshallResult<'js, MRS<'i, 'js, M>>>::marshall_result(
        &mut state, res,
    )?;
    mr.into_js(input.ctx())
}

pub fn invoke_extern_2a<'i, 'js, M>(
    mut input: InputAccessor<'i, 'js>,
    func: unsafe extern "C" fn(
        <MRS<'i, 'js, M> as MarshallArgument<'js, MRS<'i, 'js, M>, 1>>::Type,
        <MRS<'i, 'js, M> as MarshallArgument<'js, MRS<'i, 'js, M>, 2>>::Type,
    )
        -> <MRS<'i, 'js, M> as MarshallResult<'js, MRS<'i, 'js, M>>>::Type,
) -> Result<Value<'js>>
where
    M: InitMarshall<'i, 'js>,
    MRS<'i, 'js, M>: MarshallResult<'js, MRS<'i, 'js, M>>,
    MRS<'i, 'js, M>: MarshallArgument<'js, MRS<'i, 'js, M>, 1>,
    MRS<'i, 'js, M>: MarshallArgument<'js, MRS<'i, 'js, M>, 2>,
{
    let mut state = M::init_marshall_state(&mut input)?;
    let arg1 =
        <MRS<'i, 'js, M> as MarshallArgument<'js, MRS<'i, 'js, M>, 1>>::marshall_arg(&mut state)?;
    let arg2 =
        <MRS<'i, 'js, M> as MarshallArgument<'js, MRS<'i, 'js, M>, 2>>::marshall_arg(&mut state)?;
    let res = unsafe { func(arg!(arg1)?, arg!(arg2)?) };
    let mr = <MRS<'i, 'js, M> as MarshallResult<'js, MRS<'i, 'js, M>>>::marshall_result(
        &mut state, res,
    )?;
    mr.into_js(input.ctx())
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
