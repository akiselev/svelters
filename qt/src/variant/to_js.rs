use rquickjs::{Ctx, IntoJs, Value};

use crate::common::*;

pub unsafe fn qvariant_to_js<'js, T: CastInto<Ptr<QVariant>>>(
    ctx: Ctx<'js>,
    var: T,
) -> Result<Value<'js>> {
    let var: Ptr<QVariant> = var.cast_into();
    if var.is_null() {
        return Err(exception!("Could not cast to QVariant"));
    }
    match var.type_() {
        VariantType::Invalid => Err(exception!("QVariant has invalid type")),
        VariantType::Bool => var.to_bool().into_js(ctx),
        VariantType::Int => var.to_int_0a().into_js(ctx),
        VariantType::UInt => var.to_u_int_0a().into_js(ctx),
        VariantType::LongLong => var.to_long_long_0a().into_js(ctx),
        VariantType::ULongLong => var.to_u_long_long_0a().into_js(ctx),
        VariantType::Double => var.to_double_0a().into_js(ctx),
        VariantType::Char => var.to_string().to_std_string().into_js(ctx),
        VariantType::Map => {
            unimplemented!();
        }
        VariantType::List => {
            unimplemented!();
        }
        VariantType::String => var.to_string().to_std_string().into_js(ctx),
        VariantType::StringList => {
            let list = var.to_string_list();
            let output = vec![];
            let len = list.length();
            for index in 0..len {
                output.push(list.index(0).to_std_string());
            }
            output.into_js(ctx)
        }
        VariantType::ByteArray => {
            unimplemented!();
        }
        VariantType::BitArray => {
            unimplemented!();
        }
        VariantType::Date => {
            unimplemented!();
        }
        VariantType::Time => {
            unimplemented!();
        }
        VariantType::DateTime => {
            unimplemented!();
        }
        VariantType::Url => var.to_string().to_std_string().into_js(ctx),
        VariantType::Locale => {
            unimplemented!();
        }
        VariantType::Rect => {
            let val: super::types::Rect = var.to_rect().as_ref().into();
            val.into_js(ctx)
        }
        VariantType::RectF => {
            let val: super::types::RectF = var.to_rect_f().as_ref().into();
            val.into_js(ctx)
        }
        VariantType::Size => {
            let val: super::types::Size = var.to_size().as_ref().into();
            val.into_js(ctx)
        }
        VariantType::SizeF => {
            let val: super::types::SizeF = var.to_size_f().as_ref().into();
            val.into_js(ctx)
        }
        VariantType::Line => {
            let val: super::types::Line = var.to_line().as_ref().into();
            val.into_js(ctx)
        }
        VariantType::LineF => {
            let val: super::types::LineF = var.to_line_f().as_ref().into();
            val.into_js(ctx)
        }
        VariantType::Point => {
            let val: super::types::Point = var.to_point().as_ref().into();
            val.into_js(ctx)
        }
        VariantType::PointF => {
            let val: super::types::PointF = var.to_point_f().as_ref().into();
            val.into_js(ctx)
        }
        VariantType::RegExp => {
            unimplemented!();
        }
        VariantType::RegularExpression => {
            unimplemented!();
        }
        VariantType::Hash => {
            unimplemented!();
        }
        VariantType::EasingCurve => {
            unimplemented!();
        }
        VariantType::Uuid => var.to_string().to_std_string().into_js(ctx),
        VariantType::ModelIndex => {
            unimplemented!();
        }
        VariantType::PersistentModelIndex => {
            unimplemented!();
        }
        VariantType::LastCoreType => {
            unimplemented!();
        }
        VariantType::Font => {
            unimplemented!();
        }
        VariantType::Pixmap => {
            unimplemented!();
        }
        VariantType::Brush => {
            unimplemented!();
        }
        VariantType::Color => {
            unimplemented!();
        }
        VariantType::Palette => {
            unimplemented!();
        }
        VariantType::Image => {
            unimplemented!();
        }
        VariantType::Polygon => {
            unimplemented!();
        }
        VariantType::Region => {
            unimplemented!();
        }
        VariantType::Bitmap => {
            unimplemented!();
        }
        VariantType::Cursor => {
            unimplemented!();
        }
        VariantType::KeySequence => {
            unimplemented!();
        }
        VariantType::Pen => {
            unimplemented!();
        }
        VariantType::TextLength => {
            unimplemented!();
        }
        VariantType::TextFormat => {
            unimplemented!();
        }
        VariantType::Matrix => {
            unimplemented!();
        }
        VariantType::Transform => {
            unimplemented!();
        }
        VariantType::Matrix4X4 => {
            unimplemented!();
        }
        VariantType::Vector2D => {
            unimplemented!();
        }
        VariantType::Vector3D => {
            unimplemented!();
        }
        VariantType::Vector4D => {
            unimplemented!();
        }
        VariantType::Quaternion => {
            unimplemented!();
        }
        VariantType::PolygonF => {
            unimplemented!();
        }
        VariantType::Icon => {
            unimplemented!();
        }
        VariantType::LastGuiType => {
            unimplemented!();
        }
        VariantType::SizePolicy => {
            unimplemented!();
        }
        VariantType::UserType => {
            unimplemented!();
        }
        VariantType::LastType => {
            unimplemented!();
        }
        VariantType::LastCoreType2 => {
            unimplemented!();
        }
    }
}
