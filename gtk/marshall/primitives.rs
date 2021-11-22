use std::{ffi::CStr, marker::PhantomData, os::raw::c_char};

use glib::translate::ToGlibPtr;
use rquickjs::{Ctx, FromJs, Value};

use super::marshall::*;
use crate::common::*;

pub struct Primitive<T>(pub T);
impl<'a, T> Marshall<'a, T> for Primitive<T> {
    fn use_ref(&self) -> bool {
        false
    }

    fn marshall_as_owned(self) -> Result<T> {
        Ok(self.0)
    }
}

impl<'a> Marshall<'a, String> for *const c_char {
    fn marshall_as_ref(&self) -> Result<String> {
        let cstr = unsafe { CStr::from_ptr(*self) };
        let string = cstr
            .to_str()
            .map_err(|err| exception!("Could not convert CString to UTF8: {}", err))?;
        Ok(string.to_string())
    }
}

pub struct None;
pub struct Full;
pub struct Container;

pub struct SelfMarshall<T, P, KIND>(PhantomData<(T, P, KIND)>);

impl<'i, 'js, T, P, KIND> InitMarshallArg<'i, 'js, 0> for SelfMarshall<T, P, KIND>
where
    T: for<'a> ToGlibPtr<'a, *mut P> + glib::ObjectType,
    P: Copy,
{
    type Output = SelfMarshallInner<T, P, KIND>;

    fn init_marshall_arg(input: &mut rquickjs::InputAccessor<'i, 'js>) -> Result<Self::Output> {
        Ok(SelfMarshallInner(
            crate::newtype::Gtk::<T>::from_js(input.ctx(), input.arg()?)?,
            PhantomData,
        ))
    }
}

pub struct SelfMarshallInner<T, P, KIND>(crate::newtype::Gtk<T>, PhantomData<(T, P, KIND)>);

impl<'js, T, P, KIND> MarshallArgument<'js> for SelfMarshallInner<T, P, KIND>
where
    P: Copy,
    Self: for<'a> Marshall<'a, *mut P>,
    T: for<'a> ToGlibPtr<'a, *mut P>,
{
    type Type = *mut P;

    type Marshall<'a> = Self;

    fn marshall_arg<'a>(self) -> Result<Self::Marshall<'a>> {
        Ok(self)
    }
}

impl<'js, T, P> Marshall<'js, *mut P> for SelfMarshallInner<T, P, None>
where
    T: for<'a> ToGlibPtr<'a, *mut P>,
    P: Copy,
{
    fn use_ref(&self) -> bool {
        true
    }

    fn marshall_as_ref(&self) -> Result<*mut P> {
        Ok(self.0 .0.to_glib_none().0)
    }
}
