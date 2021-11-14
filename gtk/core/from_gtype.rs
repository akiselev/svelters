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
    println!("Props for {}:", cls_name);
    for param in params.into_iter() {
        let param_ty = param.value_type();
        let prop_name = param.name().to_case(Case::Camel);
        use glib::types::Type;
        if param_ty.is_a(Type::I8) {
            println!("    {} ({}): {}", prop_name, param.name(), Type::I8.name());

            ty.param::<i8>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::U8) {
            println!("    {} ({}): {}", prop_name, param.name(), Type::U8.name());

            ty.param::<u8>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::BOOL) {
            println!(
                "    {} ({}): {}",
                prop_name,
                param.name(),
                Type::BOOL.name()
            );

            ty.param::<bool>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::I32) {
            println!("    {} ({}): {}", prop_name, param.name(), Type::I32.name());

            ty.param::<i32>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::U32) {
            println!("    {} ({}): {}", prop_name, param.name(), Type::U32.name());

            ty.param::<u32>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::I_LONG) {
            println!(
                "    {} ({}): {}",
                prop_name,
                param.name(),
                Type::I_LONG.name()
            );

            ty.param::<c_long>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::U_LONG) {
            println!(
                "    {} ({}): {}",
                prop_name,
                param.name(),
                Type::U_LONG.name()
            );

            ty.param::<c_ulong>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::I64) {
            println!("    {} ({}): {}", prop_name, param.name(), Type::I64.name());

            ty.param::<i64>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::U64) {
            println!("    {} ({}): {}", prop_name, param.name(), Type::U64.name());

            ty.param::<u64>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::F32) {
            println!("    {} ({}): {}", prop_name, param.name(), Type::F32.name());

            ty.param::<f32>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::F64) {
            println!("    {} ({}): {}", prop_name, param.name(), Type::F64.name());

            ty.param::<f64>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::STRING) {
            println!("    {} ({}): {}", prop_name, param.name(), param_ty.name());

            ty.param::<String>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Vec::<String>::static_type()) {
            println!("    {} ({}): string[]", prop_name, param.name(),);

            ty.param::<Vec<String>>(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::POINTER) {
            println!(
                ">>>>>> Type::POINTER not implemented for prop {}::{}",
                cls_name, prop_name
            );
        } else if param_ty.is_a(Type::VARIANT) {
            println!(
                ">>>>>> Type::VARIANT not implemented for prop {}::{}",
                cls_name, prop_name
            );
        } else if param_ty.is_a(Type::INTERFACE) {
            println!(
                ">>>>>> Type::INTERFACE not implemented for prop {}::{}",
                cls_name, prop_name
            );
        } else if param_ty.is_a(Type::ENUM) {
            println!("    {} ({}): {}", prop_name, param.name(), param_ty.name());
            ty.enum_prop(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::FLAGS) {
            println!(
                ">>>>>> Type::FLAGS not implemented for prop {}::{}",
                cls_name, prop_name
            );
        } else if param_ty.is_a(Type::OBJECT) {
            println!("    {} ({}): {}", prop_name, param.name(), param_ty.name());
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
