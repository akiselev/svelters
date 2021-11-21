use qt_core::QString;
use rquickjs::{Ctx, FromJs, IntoJs, Value};

use crate::common::*;

pub unsafe fn qvariant_from_js<'js, T: CastInto<Ptr<QVariant>>>(
    ctx: Ctx<'js>,
    value: Value<'js>,
    ty: VariantType,
) -> Result<CppBox<QVariant>> {
    let var = QVariant::from_type(ty);
    let res = match ty {
        VariantType::Invalid => return Err(exception!("QVariant has invalid type")),
        VariantType::Bool => QVariant::from_bool(bool::from_js(ctx, value)?),
        VariantType::Int => QVariant::from_int(i32::from_js(ctx, value)?),
        VariantType::UInt => QVariant::from_uint(u32::from_js(ctx, value)?),
        VariantType::LongLong => QVariant::from_i64(i64::from_js(ctx, value)?),
        VariantType::ULongLong => QVariant::from_u64(u64::from_js(ctx, value)?),
        VariantType::Double => QVariant::from_double(f64::from_js(ctx, value)?),
        VariantType::Char => {
            QVariant::from_q_string(QString::from_std_str(&String::from_js(ctx, value)?).as_ref())
        }
        VariantType::Map => {
            unimplemented!();
        }
        VariantType::List => {
            unimplemented!();
        }
        VariantType::String => {
            QVariant::from_q_string(QString::from_std_str(&String::from_js(ctx, value)?).as_ref())
        }
        VariantType::StringList => {
            unimplemented!();
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
        VariantType::Url => unimplemented!(),
        VariantType::Locale => {
            unimplemented!();
        }
        VariantType::Rect => {
            let val: CppBox<qt_core::QRect> = super::types::Rect::from_js(ctx, value)?.into();
            QVariant::from_q_rect(val.as_ref())
        }
        VariantType::RectF => {
            let val: CppBox<qt_core::QRectF> = super::types::RectF::from_js(ctx, value)?.into();
            QVariant::from_q_rect_f(val.as_ref())
        }
        VariantType::Size => {
            let val: CppBox<qt_core::QSize> = super::types::Size::from_js(ctx, value)?.into();
            QVariant::from_q_size(val.as_ref())
        }
        VariantType::SizeF => {
            let val: CppBox<qt_core::QSizeF> = super::types::SizeF::from_js(ctx, value)?.into();
            QVariant::from_q_size_f(val.as_ref())
        }
        VariantType::Line => {
            let val: CppBox<qt_core::QLine> = super::types::Line::from_js(ctx, value)?.into();
            QVariant::from_q_line(val.as_ref())
        }
        VariantType::LineF => {
            let val: CppBox<qt_core::QLineF> = super::types::LineF::from_js(ctx, value)?.into();
            QVariant::from_q_line_f(val.as_ref())
        }
        VariantType::Point => {
            let val: CppBox<qt_core::QPoint> = super::types::Point::from_js(ctx, value)?.into();
            QVariant::from_q_point(val.as_ref())
        }
        VariantType::PointF => {
            let val: CppBox<qt_core::QPointF> = super::types::PointF::from_js(ctx, value)?.into();
            QVariant::from_q_point_f(val.as_ref())
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
        VariantType::Uuid => unimplemented!(),
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
        _ => {
            unimplemented!();
        }
    };
    Ok(res)
}
