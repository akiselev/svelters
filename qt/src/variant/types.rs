use rquickjs::{FromJs, IntoJs};

use crate::common::*;

#[derive(Copy, Clone, IntoJs, FromJs)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
}

impl From<Ref<qt_core::QRect>> for Rect {
    fn from(val: Ref<qt_core::QRect>) -> Self {
        unsafe {
            Self {
                x: val.x(),
                y: val.y(),
            }
        }
    }
}

impl From<Rect> for CppBox<qt_core::QRect> {
    fn from(val: Rect) -> Self {
        unsafe {
            let var = qt_core::QRect::new();
            var.set_x(self.x);
            var.set_y(self.y);
            var
        }
    }
}

#[derive(Copy, Clone, IntoJs, FromJs)]
pub struct RectF {
    pub x: f64,
    pub y: f64,
}

impl From<Ref<qt_core::QRectF>> for RectF {
    fn from(val: Ref<qt_core::QRectF>) -> Self {
        unsafe {
            Self {
                x: val.x(),
                y: val.y(),
            }
        }
    }
}

impl From<RectF> for CppBox<qt_core::QRectF> {
    fn from(val: RectF) -> Self {
        unsafe {
            let var = qt_core::QRectF::new();
            var.set_x(val.x);
            var.set_y(val.y);
            var
        }
    }
}

#[derive(Copy, Clone, IntoJs, FromJs)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl From<Ref<qt_core::QSize>> for Size {
    fn from(val: Ref<qt_core::QSize>) -> Self {
        unsafe {
            Self {
                width: val.width(),
                height: val.height(),
            }
        }
    }
}

impl From<Size> for CppBox<qt_core::QSize> {
    fn from(val: Size) -> Self {
        unsafe { qt_core::QSize::new_2a(val.width, val.height) }
    }
}

#[derive(Copy, Clone, IntoJs, FromJs)]
pub struct SizeF {
    pub width: f64,
    pub height: f64,
}

impl From<Ref<qt_core::QSizeF>> for SizeF {
    fn from(val: Ref<qt_core::QSizeF>) -> Self {
        unsafe {
            Self {
                width: val.width(),
                height: val.height(),
            }
        }
    }
}

impl From<SizeF> for CppBox<qt_core::QSizeF> {
    fn from(val: SizeF) -> Self {
        unsafe { qt_core::QSizeF::new_2a(val.width, val.height) }
    }
}

#[derive(Copy, Clone, IntoJs, FromJs)]
pub struct Line {
    pub x1: i32,
    pub x2: i32,
    pub y1: i32,
    pub y2: i32,
}

impl From<Ref<qt_core::QLine>> for Line {
    fn from(val: Ref<qt_core::QLine>) -> Self {
        unsafe {
            Self {
                x1: val.x1(),
                x2: val.x2(),
                y1: val.y1(),
                y2: val.y2(),
            }
        }
    }
}

impl From<Line> for CppBox<qt_core::QLine> {
    fn from(val: Line) -> Self {
        unsafe { qt_core::QLine::new_4a(val.x1, val.y1, val.x2, val.y2) }
    }
}

#[derive(Copy, Clone, IntoJs, FromJs)]
pub struct LineF {
    pub x1: f64,
    pub x2: f64,
    pub y1: f64,
    pub y2: f64,
}

impl From<Ref<qt_core::QLineF>> for LineF {
    fn from(val: Ref<qt_core::QLineF>) -> Self {
        unsafe {
            Self {
                x1: val.x1(),
                x2: val.x2(),
                y1: val.y1(),
                y2: val.y2(),
            }
        }
    }
}

impl From<LineF> for CppBox<qt_core::QLineF> {
    fn from(val: LineF) -> Self {
        unsafe { qt_core::QLineF::new_4a(val.x1, val.y1, val.x2, val.y2) }
    }
}

#[derive(Copy, Clone, IntoJs, FromJs)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl From<Ref<qt_core::QPoint>> for Point {
    fn from(val: Ref<qt_core::QPoint>) -> Self {
        unsafe {
            Self {
                x: val.x(),
                y: val.y(),
            }
        }
    }
}

impl From<Point> for CppBox<qt_core::QPoint> {
    fn from(val: Point) -> Self {
        unsafe { qt_core::QPoint::new_2a(val.x, val.y) }
    }
}

#[derive(Copy, Clone, IntoJs, FromJs)]
pub struct PointF {
    pub x: f64,
    pub y: f64,
}

impl From<Ref<qt_core::QPointF>> for PointF {
    fn from(val: Ref<qt_core::QPointF>) -> Self {
        unsafe {
            Self {
                x: val.x(),
                y: val.y(),
            }
        }
    }
}

impl From<PointF> for CppBox<qt_core::QPointF> {
    fn from(val: PointF) -> Self {
        unsafe { qt_core::QPointF::new_2a(val.x, val.y) }
    }
}

#[derive(Copy, Clone, IntoJs, FromJs)]
pub struct Vector2D {
    pub x: i32,
    pub y: i32,
}

// impl From<Ref<qt_gui::QVector2D>> for Vector2D {
//     fn from(val: Ref<qt_gui::QVector2D>) -> Self {
//         unsafe {
//             Self {
//                 x: val.x(),
//                 y: val.y(),
//             }
//         }
//     }
// }

#[derive(Copy, Clone, IntoJs, FromJs)]
pub struct Vector3D {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

// impl From<Ref<qt_gui::QVector3D>> for Vector3D {
//     fn from(val: Ref<qt_gui::QVector3D>) -> Self {
//         unsafe {
//             Self {
//                 x: val.x(),
//                 y: val.y(),
//                 z: val.z(),
//             }
//         }
//     }
// }

#[derive(Copy, Clone, IntoJs, FromJs)]
pub struct Vector4D {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub w: i32,
}

// impl From<Ref<qt_gui::QVector4D>> for Vector4D {
//     fn from(val: Ref<qt_gui::QVector4D>) -> Self {
//         unsafe {
//             Self {
//                 x: val.x(),
//                 y: val.y(),
//                 z: val.z(),
//                 w: val.w(),
//             }
//         }
//     }
// }
