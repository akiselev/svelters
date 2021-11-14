use std::os::raw::{c_long, c_ulong};

use convert_case::{Case, Casing};
use glib::{object::IsClass, Class};
use rquickjs::FromJs;

use crate::common::*;

pub fn define<'js, T, CLS>(ty: &Type<'js, CLS>) -> Result<()>
where
    T: IsClass + StaticType,
    CLS: 'static + AsObject,
    &'js CLS: FromJs<'js>,
{
    let cls = Class::<GObject>::from_type(T::static_type()).unwrap();
    let params = cls.list_properties();
    let cls_name = std::any::type_name::<T>();
    for param in params.into_iter() {
        let param_ty = param.value_type();
        let prop_name = param.name().to_case(Case::Camel);
        use glib::types::Type;
        if param_ty.is_a(Type::I8) {
            ty.param::<i8>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::U8) {
            ty.param::<u8>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::BOOL) {
            ty.param::<bool>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::I32) {
            ty.param::<i32>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::U32) {
            ty.param::<u32>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::I_LONG) {
            ty.param::<c_long>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::U_LONG) {
            ty.param::<c_ulong>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::I64) {
            ty.param::<i64>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::U64) {
            ty.param::<u64>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::F32) {
            ty.param::<f32>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::F64) {
            ty.param::<f64>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::STRING) {
            ty.param::<String>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Vec::<String>::static_type()) {
            ty.param::<Vec<String>>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::POINTER) {
            println!();
        } else if param_ty.is_a(Type::VARIANT) {
        } else if param_ty.is_a(Type::INTERFACE) {
        } else if param_ty.is_a(Type::ENUM) {
            ty.enum_prop(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::FLAGS) {
        } else if param_ty.is_a(Type::OBJECT) {
            ty.obj_prop(prop_name.as_str(), param)?;
        } else {
            panic!(
                ">>>>>> Type::??? not implemented for prop {}::{}",
                cls_name, prop_name
            );
        }
    }
    Ok(())
}
