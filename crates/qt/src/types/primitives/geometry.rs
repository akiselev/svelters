use frunk::Coprod;
use qt_core::QSize;
use rquickjs::FromJs;

use crate::inner::*;

#[derive(FromJs)]
struct QSizeObj {
    width: i32,
    height: i32,
}
type QSizeIn = Coprod![(i32, i32), QSizeObj];

impl<'js> FromJs<'js> for Q<QSize> {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        if let Some(array) = value.as_array() {
            if array.len() == 2 {
                let (width, height): (i32, i32) = (array.get(0)?, array.get(0)?);
                return Ok(Q::Box(unsafe { QSize::new_2a(width, height) }));
            }
        } else if let Some(obj) = value.as_object() {
            let (width, height): (i32, i32) = (obj.get("width")?, obj.get("height")?);
            return Ok(Q::Box(unsafe { QSize::new_2a(width, height) }));
        }
        Err(exception!(
            "QSize must be in the format [width, height] or {{width, height}}"
        ))
    }
}
