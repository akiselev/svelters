use cast_trait_object::{dyn_cast, dyn_upcast, DynCastExt};
use cpp::{cpp, cpp_class};

use crate::{button::*, widget::*};

cpp! {{
    #include <memory>
    #include <tuple>
    #include <QtWidgets/QPushButton>
    #include <QtWidgets/QAbstractButton>
    #include <QtCore/QPointer>
    #include <QtCore/QObject>
}}

// #[dyn_cast(IQWidget, IQAbstractButton, IQPushButton)]
// #[dyn_upcast]
// pub trait IQObject {
//     fn as_qt_ref(&self) -> Option<QObject>;
// }

// #[dyn_cast(IQWidget, IQAbstractButton, IQPushButton)]
// #[dyn_upcast]
// impl IQObject for QObjectPtr {
//     fn as_qt_ref(&self) -> Option<QObject> {
//         let (is_safe, ptr) = cpp!(unsafe [self as "QPointer<QObject>*"] -> (bool, QObject) as "std::tuple<bool, QObject*>" {
//             if (self->isNull()) {
//                 return std::make_tuple(false, (QObject*) NULL);
//             } else {
//                 return std::make_tuple(true, qobject_cast<QObject*>(*self));
//             }
//         });
//         match is_safe {
//             true => Some(ptr),
//             false => None,
//         }
//     }
// }

// #[dyn_cast(IQWidget, IQAbstractButton, IQPushButton)]
// #[dyn_upcast]
// impl IQObject for QWidgetPtr {
//     fn as_qt_ref(&self) -> Option<QObject> {
//         let (is_safe, ptr) = cpp!(unsafe [self as "QPointer<QWidget>*"] -> (bool, QObject) as "std::tuple<bool, QObject*>" {
//             if (self->isNull()) {
//                 return std::make_tuple(false, (QObject*) NULL);
//             } else {
//                 return std::make_tuple(true, qobject_cast<QObject*>(*self));
//             }
//         });
//         match is_safe {
//             true => Some(ptr),
//             false => None,
//         }
//     }
// }

// pub trait IQWidget: IQObject {
//     fn as_qt_ref(&self) -> QWidget;
// }

// impl IQWidget for QWidgetPtr {
//     fn as_qt_ref(&self) -> QWidget {
//         cpp!(unsafe [self as "QPointer<QWidget>*"] -> QWidget as "QWidget*" {
//             return qobject_cast<QWidget*>(*self);
//         })
//     }
// }

// pub trait IQAbstractButton: IQWidget {
//     fn as_qt_ref(&self) -> QAbstractButtonPtr;
// }

// pub trait IQPushButton: IQAbstractButton {
//     fn as_qt_ref(&self) -> QPushButtonPtr;
// }
