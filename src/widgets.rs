use crate::{types::*, QObject, QObjectPtr, QRef, QWidget, QWidgetPtr};
use cast_trait_object::{dyn_cast, dyn_upcast, DynCastExt};
use cpp::{cpp, cpp_class};
use qttypes::*;

cpp! {{
#include <QtWidgets/QTabWidget>
#include <QtWidgets/QMdiSubWindow>
#include <QtWidgets/QDockWidget>
#include <QtWidgets/QMainWindow>
#include <QtWidgets/QStatusBar>
#include <QtWidgets/QListView>
#include <QtWidgets/QAbstractItemView>
#include <QtWidgets/QDialog>
#include <QtWidgets/QOpenGLWidget>
#include <QtWidgets/QDateTimeEdit>
#include <QtWidgets/QTableWidget>
#include <QtWidgets/QMenu>
#include <QtWidgets/QSpinBox>
#include <QtWidgets/QKeySequenceEdit>
#include <QtWidgets/QStackedWidget>
#include <QtWidgets/QColumnView>
#include <QtWidgets/QMenuBar>
#include <QtWidgets/QDoubleSpinBox>
#include <QtWidgets/QWizardPage>
#include <QtWidgets/QCommandLinkButton>
#include <QtWidgets/QCheckBox>
#include <QtWidgets/QUndoView>
#include <QtWidgets/QTableView>
#include <QtWidgets/QGraphicsView>
#include <QtWidgets/QDialogButtonBox>
#include <QtWidgets/QWizard>
#include <QtWidgets/QMdiArea>
#include <QtWidgets/QPlainTextEdit>
#include <QtWidgets/QSplitterHandle>
#include <QtWidgets/QLCDNumber>
#include <QtWidgets/QProgressDialog>
#include <QtWidgets/QAbstractScrollArea>
#include <QtWidgets/QFileDialog>
#include <QtWidgets/QRubberBand>
#include <QtWidgets/QColorDialog>
#include <QtWidgets/QAbstractSpinBox>
#include <QtWidgets/QSplashScreen>
#include <QtWidgets/QDial>
#include <QtWidgets/QToolButton>
#include <QtWidgets/QFocusFrame>
#include <QtWidgets/QTimeEdit>
#include <QtWidgets/QComboBox>
#include <QtWidgets/QRadioButton>
#include <QtWidgets/QToolBox>
#include <QtWidgets/QAbstractSlider>
#include <QtWidgets/QTextEdit>
#include <QtWidgets/QSlider>
#include <QtWidgets/QInputDialog>
#include <QtWidgets/QScrollArea>
#include <QtWidgets/QMessageBox>
#include <QtWidgets/QSplitter>
#include <QtWidgets/QDateEdit>
#include <QtWidgets/QTextBrowser>
#include <QtWidgets/QSizeGrip>
#include <QtWidgets/QFontComboBox>
#include <QtWidgets/QErrorMessage>
#include <QtWidgets/QFrame>
#include <QtWidgets/QProgressBar>
#include <QtWidgets/QGroupBox>
#include <QtWidgets/QHeaderView>
#include <QtWidgets/QLabel>
#include <QtWidgets/QPushButton>
#include <QtWidgets/QAbstractButton>
#include <QtWidgets/QTabBar>
#include <QtWidgets/QScrollBar>
#include <QtWidgets/QTreeView>
#include <QtWidgets/QListWidget>
#include <QtWidgets/QFontDialog>
#include <QtWidgets/QLineEdit>
#include <QtWidgets/QToolBar>
#include <QtWidgets/QTreeWidget>
#include <QtWidgets/QCalendarWidget>
#include <memory>
#include <tuple>
#include <QtCore/QPointer>
#include <QtCore/QObject>
}}

#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
pub trait IQObject {
    unsafe fn as_qt_ref(&self) -> Option<QObject>;
}
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QObjectPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QObject>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQWidget: IQObject {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>>;
}
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QWidget>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
cpp_class ! (pub unsafe struct QRadioButtonPtr as "QPointer<QRadioButton>");
cpp_class ! (pub unsafe struct QRadioButton as "QRadioButton*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QRadioButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QRadioButton>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQRadioButton: IQAbstractButton {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QRadioButton>>;
}
impl QRadioButton {
    pub fn new() -> Option<QRadioButtonPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QRadioButtonPtr) as "std::tuple<bool, QPointer<QRadioButton>>" { QPointer < QRadioButton > btn = new QRadioButton () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
cpp_class ! (pub unsafe struct QStackedWidgetPtr as "QPointer<QStackedWidget>");
cpp_class ! (pub unsafe struct QStackedWidget as "QStackedWidget*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QStackedWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QStackedWidget>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQStackedWidget: IQFrame {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QStackedWidget>>;
}
impl QStackedWidget {
    pub fn new() -> Option<QStackedWidgetPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QStackedWidgetPtr) as "std::tuple<bool, QPointer<QStackedWidget>>" { QPointer < QStackedWidget > btn = new QStackedWidget () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QStackedWidget> {
    pub fn currentIndex(self) -> int {
        cpp ! (unsafe [self as "QStackedWidget*"] -> int as "int" { return self -> currentIndex () ; })
    }
    pub fn setCurrentIndex(self, value: int) {
        let value = cpp ! (unsafe [self as "QStackedWidget*" , value as "int"] -> int as "int" { self -> setCurrentIndex (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QGroupBoxPtr as "QPointer<QGroupBox>");
cpp_class ! (pub unsafe struct QGroupBox as "QGroupBox*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QGroupBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QGroupBox>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQGroupBox: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QGroupBox>>;
}
impl QGroupBox {
    pub fn new() -> Option<QGroupBoxPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QGroupBoxPtr) as "std::tuple<bool, QPointer<QGroupBox>>" { QPointer < QGroupBox > btn = new QGroupBox () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QGroupBox> {
    pub fn isCheckable(self) -> bool {
        cpp ! (unsafe [self as "QGroupBox*"] -> bool as "bool" { return self -> isCheckable () ; })
    }
    pub fn setCheckable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QGroupBox*" , value as "bool"] -> bool as "bool" { self -> setCheckable (value) ; return value ; });
    }
    pub fn isChecked(self) -> bool {
        cpp ! (unsafe [self as "QGroupBox*"] -> bool as "bool" { return self -> isChecked () ; })
    }
    pub fn setChecked(self, value: bool) {
        let value = cpp ! (unsafe [self as "QGroupBox*" , value as "bool"] -> bool as "bool" { self -> setChecked (value) ; return value ; });
    }
    pub fn isFlat(self) -> bool {
        cpp ! (unsafe [self as "QGroupBox*"] -> bool as "bool" { return self -> isFlat () ; })
    }
    pub fn setFlat(self, value: bool) {
        let value = cpp ! (unsafe [self as "QGroupBox*" , value as "bool"] -> bool as "bool" { self -> setFlat (value) ; return value ; });
    }
    pub fn title(self) -> QString {
        cpp ! (unsafe [self as "QGroupBox*"] -> QString as "QString" { return QString (self -> title ()) ; })
    }
    pub fn setTitle(self, value: QString) {
        let value = cpp ! (unsafe [self as "QGroupBox*" , value as "QString"] -> QString as "QString" { self -> setTitle (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QSpinBoxPtr as "QPointer<QSpinBox>");
cpp_class ! (pub unsafe struct QSpinBox as "QSpinBox*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QSpinBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSpinBox>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQSpinBox: IQAbstractSpinBox {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QSpinBox>>;
}
impl QSpinBox {
    pub fn new() -> Option<QSpinBoxPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QSpinBoxPtr) as "std::tuple<bool, QPointer<QSpinBox>>" { QPointer < QSpinBox > btn = new QSpinBox () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QSpinBox> {
    pub fn displayIntegerBase(self) -> int {
        cpp ! (unsafe [self as "QSpinBox*"] -> int as "int" { return self -> displayIntegerBase () ; })
    }
    pub fn setDisplayIntegerBase(self, value: int) {
        let value = cpp ! (unsafe [self as "QSpinBox*" , value as "int"] -> int as "int" { self -> setDisplayIntegerBase (value) ; return value ; });
    }
    pub fn maximum(self) -> int {
        cpp ! (unsafe [self as "QSpinBox*"] -> int as "int" { return self -> maximum () ; })
    }
    pub fn setMaximum(self, value: int) {
        let value = cpp ! (unsafe [self as "QSpinBox*" , value as "int"] -> int as "int" { self -> setMaximum (value) ; return value ; });
    }
    pub fn minimum(self) -> int {
        cpp ! (unsafe [self as "QSpinBox*"] -> int as "int" { return self -> minimum () ; })
    }
    pub fn setMinimum(self, value: int) {
        let value = cpp ! (unsafe [self as "QSpinBox*" , value as "int"] -> int as "int" { self -> setMinimum (value) ; return value ; });
    }
    pub fn prefix(self) -> QString {
        cpp ! (unsafe [self as "QSpinBox*"] -> QString as "QString" { return QString (self -> prefix ()) ; })
    }
    pub fn setPrefix(self, value: QString) {
        let value = cpp ! (unsafe [self as "QSpinBox*" , value as "QString"] -> QString as "QString" { self -> setPrefix (value) ; return value ; });
    }
    pub fn singleStep(self) -> int {
        cpp ! (unsafe [self as "QSpinBox*"] -> int as "int" { return self -> singleStep () ; })
    }
    pub fn setSingleStep(self, value: int) {
        let value = cpp ! (unsafe [self as "QSpinBox*" , value as "int"] -> int as "int" { self -> setSingleStep (value) ; return value ; });
    }
    pub fn suffix(self) -> QString {
        cpp ! (unsafe [self as "QSpinBox*"] -> QString as "QString" { return QString (self -> suffix ()) ; })
    }
    pub fn setSuffix(self, value: QString) {
        let value = cpp ! (unsafe [self as "QSpinBox*" , value as "QString"] -> QString as "QString" { self -> setSuffix (value) ; return value ; });
    }
    pub fn value(self) -> int {
        cpp ! (unsafe [self as "QSpinBox*"] -> int as "int" { return self -> value () ; })
    }
    pub fn setValue(self, value: int) {
        let value = cpp ! (unsafe [self as "QSpinBox*" , value as "int"] -> int as "int" { self -> setValue (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QCalendarWidgetPtr as "QPointer<QCalendarWidget>");
cpp_class ! (pub unsafe struct QCalendarWidget as "QCalendarWidget*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QCalendarWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QCalendarWidget>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQCalendarWidget: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QCalendarWidget>>;
}
impl QCalendarWidget {
    pub fn new() -> Option<QCalendarWidgetPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QCalendarWidgetPtr) as "std::tuple<bool, QPointer<QCalendarWidget>>" { QPointer < QCalendarWidget > btn = new QCalendarWidget () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QCalendarWidget> {
    pub fn dateEditAcceptDelay(self) -> int {
        cpp ! (unsafe [self as "QCalendarWidget*"] -> int as "int" { return self -> dateEditAcceptDelay () ; })
    }
    pub fn setDateEditAcceptDelay(self, value: int) {
        let value = cpp ! (unsafe [self as "QCalendarWidget*" , value as "int"] -> int as "int" { self -> setDateEditAcceptDelay (value) ; return value ; });
    }
    pub fn isDateEditEnabled(self) -> bool {
        cpp ! (unsafe [self as "QCalendarWidget*"] -> bool as "bool" { return self -> isDateEditEnabled () ; })
    }
    pub fn setDateEditEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QCalendarWidget*" , value as "bool"] -> bool as "bool" { self -> setDateEditEnabled (value) ; return value ; });
    }
    pub fn isGridVisible(self) -> bool {
        cpp ! (unsafe [self as "QCalendarWidget*"] -> bool as "bool" { return self -> isGridVisible () ; })
    }
    pub fn setGridVisible(self, value: bool) {
        let value = cpp ! (unsafe [self as "QCalendarWidget*" , value as "bool"] -> bool as "bool" { self -> setGridVisible (value) ; return value ; });
    }
    pub fn maximumDate(self) -> QDate {
        cpp ! (unsafe [self as "QCalendarWidget*"] -> QDate as "QDate" { return QDate (self -> maximumDate ()) ; })
    }
    pub fn setMaximumDate(self, value: QDate) {
        let value = cpp ! (unsafe [self as "QCalendarWidget*" , value as "QDate"] -> QDate as "QDate" { self -> setMaximumDate (value) ; return value ; });
    }
    pub fn minimumDate(self) -> QDate {
        cpp ! (unsafe [self as "QCalendarWidget*"] -> QDate as "QDate" { return QDate (self -> minimumDate ()) ; })
    }
    pub fn setMinimumDate(self, value: QDate) {
        let value = cpp ! (unsafe [self as "QCalendarWidget*" , value as "QDate"] -> QDate as "QDate" { self -> setMinimumDate (value) ; return value ; });
    }
    pub fn isNavigationBarVisible(self) -> bool {
        cpp ! (unsafe [self as "QCalendarWidget*"] -> bool as "bool" { return self -> isNavigationBarVisible () ; })
    }
    pub fn setNavigationBarVisible(self, value: bool) {
        let value = cpp ! (unsafe [self as "QCalendarWidget*" , value as "bool"] -> bool as "bool" { self -> setNavigationBarVisible (value) ; return value ; });
    }
    pub fn selectedDate(self) -> QDate {
        cpp ! (unsafe [self as "QCalendarWidget*"] -> QDate as "QDate" { return QDate (self -> selectedDate ()) ; })
    }
    pub fn setSelectedDate(self, value: QDate) {
        let value = cpp ! (unsafe [self as "QCalendarWidget*" , value as "QDate"] -> QDate as "QDate" { self -> setSelectedDate (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QDateEditPtr as "QPointer<QDateEdit>");
cpp_class ! (pub unsafe struct QDateEdit as "QDateEdit*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QDateEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDateEdit>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQDateEdit: IQDateTimeEdit {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDateEdit>>;
}
impl QDateEdit {
    pub fn new() -> Option<QDateEditPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QDateEditPtr) as "std::tuple<bool, QPointer<QDateEdit>>" { QPointer < QDateEdit > btn = new QDateEdit () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
cpp_class ! (pub unsafe struct QPlainTextEditPtr as "QPointer<QPlainTextEdit>");
cpp_class ! (pub unsafe struct QPlainTextEdit as "QPlainTextEdit*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QPlainTextEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QPlainTextEdit>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQPlainTextEdit: IQAbstractScrollArea {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QPlainTextEdit>>;
}
impl QPlainTextEdit {
    pub fn new() -> Option<QPlainTextEditPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QPlainTextEditPtr) as "std::tuple<bool, QPointer<QPlainTextEdit>>" { QPointer < QPlainTextEdit > btn = new QPlainTextEdit () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QPlainTextEdit> {
    pub fn backgroundVisible(self) -> bool {
        cpp ! (unsafe [self as "QPlainTextEdit*"] -> bool as "bool" { return self -> backgroundVisible () ; })
    }
    pub fn setBackgroundVisible(self, value: bool) {
        let value = cpp ! (unsafe [self as "QPlainTextEdit*" , value as "bool"] -> bool as "bool" { self -> setBackgroundVisible (value) ; return value ; });
    }
    pub fn centerOnScroll(self) -> bool {
        cpp ! (unsafe [self as "QPlainTextEdit*"] -> bool as "bool" { return self -> centerOnScroll () ; })
    }
    pub fn setCenterOnScroll(self, value: bool) {
        let value = cpp ! (unsafe [self as "QPlainTextEdit*" , value as "bool"] -> bool as "bool" { self -> setCenterOnScroll (value) ; return value ; });
    }
    pub fn cursorWidth(self) -> int {
        cpp ! (unsafe [self as "QPlainTextEdit*"] -> int as "int" { return self -> cursorWidth () ; })
    }
    pub fn setCursorWidth(self, value: int) {
        let value = cpp ! (unsafe [self as "QPlainTextEdit*" , value as "int"] -> int as "int" { self -> setCursorWidth (value) ; return value ; });
    }
    pub fn documentTitle(self) -> QString {
        cpp ! (unsafe [self as "QPlainTextEdit*"] -> QString as "QString" { return QString (self -> documentTitle ()) ; })
    }
    pub fn setDocumentTitle(self, value: QString) {
        let value = cpp ! (unsafe [self as "QPlainTextEdit*" , value as "QString"] -> QString as "QString" { self -> setDocumentTitle (value) ; return value ; });
    }
    pub fn maximumBlockCount(self) -> int {
        cpp ! (unsafe [self as "QPlainTextEdit*"] -> int as "int" { return self -> maximumBlockCount () ; })
    }
    pub fn setMaximumBlockCount(self, value: int) {
        let value = cpp ! (unsafe [self as "QPlainTextEdit*" , value as "int"] -> int as "int" { self -> setMaximumBlockCount (value) ; return value ; });
    }
    pub fn overwriteMode(self) -> bool {
        cpp ! (unsafe [self as "QPlainTextEdit*"] -> bool as "bool" { return self -> overwriteMode () ; })
    }
    pub fn setOverwriteMode(self, value: bool) {
        let value = cpp ! (unsafe [self as "QPlainTextEdit*" , value as "bool"] -> bool as "bool" { self -> setOverwriteMode (value) ; return value ; });
    }
    pub fn placeholderText(self) -> QString {
        cpp ! (unsafe [self as "QPlainTextEdit*"] -> QString as "QString" { return QString (self -> placeholderText ()) ; })
    }
    pub fn setPlaceholderText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QPlainTextEdit*" , value as "QString"] -> QString as "QString" { self -> setPlaceholderText (value) ; return value ; });
    }
    pub fn toPlainText(self) -> QString {
        cpp ! (unsafe [self as "QPlainTextEdit*"] -> QString as "QString" { return QString (self -> toPlainText ()) ; })
    }
    pub fn setPlainText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QPlainTextEdit*" , value as "QString"] -> QString as "QString" { self -> setPlainText (value) ; return value ; });
    }
    pub fn isReadOnly(self) -> bool {
        cpp ! (unsafe [self as "QPlainTextEdit*"] -> bool as "bool" { return self -> isReadOnly () ; })
    }
    pub fn setReadOnly(self, value: bool) {
        let value = cpp ! (unsafe [self as "QPlainTextEdit*" , value as "bool"] -> bool as "bool" { self -> setReadOnly (value) ; return value ; });
    }
    pub fn tabChangesFocus(self) -> bool {
        cpp ! (unsafe [self as "QPlainTextEdit*"] -> bool as "bool" { return self -> tabChangesFocus () ; })
    }
    pub fn setTabChangesFocus(self, value: bool) {
        let value = cpp ! (unsafe [self as "QPlainTextEdit*" , value as "bool"] -> bool as "bool" { self -> setTabChangesFocus (value) ; return value ; });
    }
    pub fn tabStopDistance(self) -> qreal {
        cpp ! (unsafe [self as "QPlainTextEdit*"] -> qreal as "qreal" { return self -> tabStopDistance () ; })
    }
    pub fn setTabStopDistance(self, value: qreal) {
        let value = cpp ! (unsafe [self as "QPlainTextEdit*" , value as "qreal"] -> qreal as "qreal" { self -> setTabStopDistance (value) ; return value ; });
    }
    pub fn isUndoRedoEnabled(self) -> bool {
        cpp ! (unsafe [self as "QPlainTextEdit*"] -> bool as "bool" { return self -> isUndoRedoEnabled () ; })
    }
    pub fn setUndoRedoEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QPlainTextEdit*" , value as "bool"] -> bool as "bool" { self -> setUndoRedoEnabled (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QMenuPtr as "QPointer<QMenu>");
cpp_class ! (pub unsafe struct QMenu as "QMenu*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QMenuPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMenu>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQMenu: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QMenu>>;
}
impl QMenu {
    pub fn new() -> Option<QMenuPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QMenuPtr) as "std::tuple<bool, QPointer<QMenu>>" { QPointer < QMenu > btn = new QMenu () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QMenu> {
    pub fn separatorsCollapsible(self) -> bool {
        cpp ! (unsafe [self as "QMenu*"] -> bool as "bool" { return self -> separatorsCollapsible () ; })
    }
    pub fn setSeparatorsCollapsible(self, value: bool) {
        let value = cpp ! (unsafe [self as "QMenu*" , value as "bool"] -> bool as "bool" { self -> setSeparatorsCollapsible (value) ; return value ; });
    }
    pub fn isTearOffEnabled(self) -> bool {
        cpp ! (unsafe [self as "QMenu*"] -> bool as "bool" { return self -> isTearOffEnabled () ; })
    }
    pub fn setTearOffEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QMenu*" , value as "bool"] -> bool as "bool" { self -> setTearOffEnabled (value) ; return value ; });
    }
    pub fn title(self) -> QString {
        cpp ! (unsafe [self as "QMenu*"] -> QString as "QString" { return QString (self -> title ()) ; })
    }
    pub fn setTitle(self, value: QString) {
        let value = cpp ! (unsafe [self as "QMenu*" , value as "QString"] -> QString as "QString" { self -> setTitle (value) ; return value ; });
    }
    pub fn toolTipsVisible(self) -> bool {
        cpp ! (unsafe [self as "QMenu*"] -> bool as "bool" { return self -> toolTipsVisible () ; })
    }
    pub fn setToolTipsVisible(self, value: bool) {
        let value = cpp ! (unsafe [self as "QMenu*" , value as "bool"] -> bool as "bool" { self -> setToolTipsVisible (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QDateTimeEditPtr as "QPointer<QDateTimeEdit>");
cpp_class ! (pub unsafe struct QDateTimeEdit as "QDateTimeEdit*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QDateTimeEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDateTimeEdit>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQDateTimeEdit: IQAbstractSpinBox {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDateTimeEdit>>;
}
impl QDateTimeEdit {
    pub fn new() -> Option<QDateTimeEditPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QDateTimeEditPtr) as "std::tuple<bool, QPointer<QDateTimeEdit>>" { QPointer < QDateTimeEdit > btn = new QDateTimeEdit () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QDateTimeEdit> {
    pub fn calendarPopup(self) -> bool {
        cpp ! (unsafe [self as "QDateTimeEdit*"] -> bool as "bool" { return self -> calendarPopup () ; })
    }
    pub fn setCalendarPopup(self, value: bool) {
        let value = cpp ! (unsafe [self as "QDateTimeEdit*" , value as "bool"] -> bool as "bool" { self -> setCalendarPopup (value) ; return value ; });
    }
    pub fn currentSectionIndex(self) -> int {
        cpp ! (unsafe [self as "QDateTimeEdit*"] -> int as "int" { return self -> currentSectionIndex () ; })
    }
    pub fn setCurrentSectionIndex(self, value: int) {
        let value = cpp ! (unsafe [self as "QDateTimeEdit*" , value as "int"] -> int as "int" { self -> setCurrentSectionIndex (value) ; return value ; });
    }
    pub fn date(self) -> QDate {
        cpp ! (unsafe [self as "QDateTimeEdit*"] -> QDate as "QDate" { return QDate (self -> date ()) ; })
    }
    pub fn setDate(self, value: QDate) {
        let value = cpp ! (unsafe [self as "QDateTimeEdit*" , value as "QDate"] -> QDate as "QDate" { self -> setDate (value) ; return value ; });
    }
    pub fn dateTime(self) -> QDateTime {
        cpp ! (unsafe [self as "QDateTimeEdit*"] -> QDateTime as "QDateTime" { return QDateTime (self -> dateTime ()) ; })
    }
    pub fn setDateTime(self, value: QDateTime) {
        let value = cpp ! (unsafe [self as "QDateTimeEdit*" , value as "QDateTime"] -> QDateTime as "QDateTime" { self -> setDateTime (value) ; return value ; });
    }
    pub fn displayFormat(self) -> QString {
        cpp ! (unsafe [self as "QDateTimeEdit*"] -> QString as "QString" { return QString (self -> displayFormat ()) ; })
    }
    pub fn setDisplayFormat(self, value: QString) {
        let value = cpp ! (unsafe [self as "QDateTimeEdit*" , value as "QString"] -> QString as "QString" { self -> setDisplayFormat (value) ; return value ; });
    }
    pub fn maximumDate(self) -> QDate {
        cpp ! (unsafe [self as "QDateTimeEdit*"] -> QDate as "QDate" { return QDate (self -> maximumDate ()) ; })
    }
    pub fn setMaximumDate(self, value: QDate) {
        let value = cpp ! (unsafe [self as "QDateTimeEdit*" , value as "QDate"] -> QDate as "QDate" { self -> setMaximumDate (value) ; return value ; });
    }
    pub fn maximumDateTime(self) -> QDateTime {
        cpp ! (unsafe [self as "QDateTimeEdit*"] -> QDateTime as "QDateTime" { return QDateTime (self -> maximumDateTime ()) ; })
    }
    pub fn setMaximumDateTime(self, value: QDateTime) {
        let value = cpp ! (unsafe [self as "QDateTimeEdit*" , value as "QDateTime"] -> QDateTime as "QDateTime" { self -> setMaximumDateTime (value) ; return value ; });
    }
    pub fn maximumTime(self) -> QTime {
        cpp ! (unsafe [self as "QDateTimeEdit*"] -> QTime as "QTime" { return QTime (self -> maximumTime ()) ; })
    }
    pub fn setMaximumTime(self, value: QTime) {
        let value = cpp ! (unsafe [self as "QDateTimeEdit*" , value as "QTime"] -> QTime as "QTime" { self -> setMaximumTime (value) ; return value ; });
    }
    pub fn minimumDate(self) -> QDate {
        cpp ! (unsafe [self as "QDateTimeEdit*"] -> QDate as "QDate" { return QDate (self -> minimumDate ()) ; })
    }
    pub fn setMinimumDate(self, value: QDate) {
        let value = cpp ! (unsafe [self as "QDateTimeEdit*" , value as "QDate"] -> QDate as "QDate" { self -> setMinimumDate (value) ; return value ; });
    }
    pub fn minimumDateTime(self) -> QDateTime {
        cpp ! (unsafe [self as "QDateTimeEdit*"] -> QDateTime as "QDateTime" { return QDateTime (self -> minimumDateTime ()) ; })
    }
    pub fn setMinimumDateTime(self, value: QDateTime) {
        let value = cpp ! (unsafe [self as "QDateTimeEdit*" , value as "QDateTime"] -> QDateTime as "QDateTime" { self -> setMinimumDateTime (value) ; return value ; });
    }
    pub fn minimumTime(self) -> QTime {
        cpp ! (unsafe [self as "QDateTimeEdit*"] -> QTime as "QTime" { return QTime (self -> minimumTime ()) ; })
    }
    pub fn setMinimumTime(self, value: QTime) {
        let value = cpp ! (unsafe [self as "QDateTimeEdit*" , value as "QTime"] -> QTime as "QTime" { self -> setMinimumTime (value) ; return value ; });
    }
    pub fn time(self) -> QTime {
        cpp ! (unsafe [self as "QDateTimeEdit*"] -> QTime as "QTime" { return QTime (self -> time ()) ; })
    }
    pub fn setTime(self, value: QTime) {
        let value = cpp ! (unsafe [self as "QDateTimeEdit*" , value as "QTime"] -> QTime as "QTime" { self -> setTime (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QRubberBandPtr as "QPointer<QRubberBand>");
cpp_class ! (pub unsafe struct QRubberBand as "QRubberBand*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QRubberBandPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QRubberBand>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQRubberBand: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QRubberBand>>;
}
cpp_class ! (pub unsafe struct QLabelPtr as "QPointer<QLabel>");
cpp_class ! (pub unsafe struct QLabel as "QLabel*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QLabelPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QLabel>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQLabel: IQFrame {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QLabel>>;
}
impl QLabel {
    pub fn new() -> Option<QLabelPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QLabelPtr) as "std::tuple<bool, QPointer<QLabel>>" { QPointer < QLabel > btn = new QLabel () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QLabel> {
    pub fn indent(self) -> int {
        cpp ! (unsafe [self as "QLabel*"] -> int as "int" { return self -> indent () ; })
    }
    pub fn setIndent(self, value: int) {
        let value = cpp ! (unsafe [self as "QLabel*" , value as "int"] -> int as "int" { self -> setIndent (value) ; return value ; });
    }
    pub fn margin(self) -> int {
        cpp ! (unsafe [self as "QLabel*"] -> int as "int" { return self -> margin () ; })
    }
    pub fn setMargin(self, value: int) {
        let value = cpp ! (unsafe [self as "QLabel*" , value as "int"] -> int as "int" { self -> setMargin (value) ; return value ; });
    }
    pub fn openExternalLinks(self) -> bool {
        cpp ! (unsafe [self as "QLabel*"] -> bool as "bool" { return self -> openExternalLinks () ; })
    }
    pub fn setOpenExternalLinks(self, value: bool) {
        let value = cpp ! (unsafe [self as "QLabel*" , value as "bool"] -> bool as "bool" { self -> setOpenExternalLinks (value) ; return value ; });
    }
    pub fn hasScaledContents(self) -> bool {
        cpp ! (unsafe [self as "QLabel*"] -> bool as "bool" { return self -> hasScaledContents () ; })
    }
    pub fn setScaledContents(self, value: bool) {
        let value = cpp ! (unsafe [self as "QLabel*" , value as "bool"] -> bool as "bool" { self -> setScaledContents (value) ; return value ; });
    }
    pub fn text(self) -> QString {
        cpp ! (unsafe [self as "QLabel*"] -> QString as "QString" { return QString (self -> text ()) ; })
    }
    pub fn setText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QLabel*" , value as "QString"] -> QString as "QString" { self -> setText (value) ; return value ; });
    }
    pub fn wordWrap(self) -> bool {
        cpp ! (unsafe [self as "QLabel*"] -> bool as "bool" { return self -> wordWrap () ; })
    }
    pub fn setWordWrap(self, value: bool) {
        let value = cpp ! (unsafe [self as "QLabel*" , value as "bool"] -> bool as "bool" { self -> setWordWrap (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QLCDNumberPtr as "QPointer<QLCDNumber>");
cpp_class ! (pub unsafe struct QLCDNumber as "QLCDNumber*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QLCDNumberPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QLCDNumber>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQLCDNumber: IQFrame {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QLCDNumber>>;
}
impl QLCDNumber {
    pub fn new() -> Option<QLCDNumberPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QLCDNumberPtr) as "std::tuple<bool, QPointer<QLCDNumber>>" { QPointer < QLCDNumber > btn = new QLCDNumber () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QLCDNumber> {
    pub fn digitCount(self) -> int {
        cpp ! (unsafe [self as "QLCDNumber*"] -> int as "int" { return self -> digitCount () ; })
    }
    pub fn setDigitCount(self, value: int) {
        let value = cpp ! (unsafe [self as "QLCDNumber*" , value as "int"] -> int as "int" { self -> setDigitCount (value) ; return value ; });
    }
    pub fn intValue(self) -> int {
        cpp ! (unsafe [self as "QLCDNumber*"] -> int as "int" { return self -> intValue () ; })
    }
    pub fn display(self, value: int) {
        let value = cpp ! (unsafe [self as "QLCDNumber*" , value as "int"] -> int as "int" { self -> display (value) ; return value ; });
    }
    pub fn smallDecimalPoint(self) -> bool {
        cpp ! (unsafe [self as "QLCDNumber*"] -> bool as "bool" { return self -> smallDecimalPoint () ; })
    }
    pub fn setSmallDecimalPoint(self, value: bool) {
        let value = cpp ! (unsafe [self as "QLCDNumber*" , value as "bool"] -> bool as "bool" { self -> setSmallDecimalPoint (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QListViewPtr as "QPointer<QListView>");
cpp_class ! (pub unsafe struct QListView as "QListView*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QListViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QListView>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQListView: IQAbstractItemView {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QListView>>;
}
impl QListView {
    pub fn new() -> Option<QListViewPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QListViewPtr) as "std::tuple<bool, QPointer<QListView>>" { QPointer < QListView > btn = new QListView () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QListView> {
    pub fn batchSize(self) -> int {
        cpp ! (unsafe [self as "QListView*"] -> int as "int" { return self -> batchSize () ; })
    }
    pub fn setBatchSize(self, value: int) {
        let value = cpp ! (unsafe [self as "QListView*" , value as "int"] -> int as "int" { self -> setBatchSize (value) ; return value ; });
    }
    pub fn gridSize(self) -> QSize {
        cpp ! (unsafe [self as "QListView*"] -> QSize as "QSize" { return QSize (self -> gridSize ()) ; })
    }
    pub fn setGridSize(self, value: QSize) {
        let value = cpp ! (unsafe [self as "QListView*" , value as "QSize"] -> QSize as "QSize" { self -> setGridSize (value) ; return value ; });
    }
    pub fn isWrapping(self) -> bool {
        cpp ! (unsafe [self as "QListView*"] -> bool as "bool" { return self -> isWrapping () ; })
    }
    pub fn setWrapping(self, value: bool) {
        let value = cpp ! (unsafe [self as "QListView*" , value as "bool"] -> bool as "bool" { self -> setWrapping (value) ; return value ; });
    }
    pub fn modelColumn(self) -> int {
        cpp ! (unsafe [self as "QListView*"] -> int as "int" { return self -> modelColumn () ; })
    }
    pub fn setModelColumn(self, value: int) {
        let value = cpp ! (unsafe [self as "QListView*" , value as "int"] -> int as "int" { self -> setModelColumn (value) ; return value ; });
    }
    pub fn isSelectionRectVisible(self) -> bool {
        cpp ! (unsafe [self as "QListView*"] -> bool as "bool" { return self -> isSelectionRectVisible () ; })
    }
    pub fn setSelectionRectVisible(self, value: bool) {
        let value = cpp ! (unsafe [self as "QListView*" , value as "bool"] -> bool as "bool" { self -> setSelectionRectVisible (value) ; return value ; });
    }
    pub fn spacing(self) -> int {
        cpp ! (unsafe [self as "QListView*"] -> int as "int" { return self -> spacing () ; })
    }
    pub fn setSpacing(self, value: int) {
        let value = cpp ! (unsafe [self as "QListView*" , value as "int"] -> int as "int" { self -> setSpacing (value) ; return value ; });
    }
    pub fn uniformItemSizes(self) -> bool {
        cpp ! (unsafe [self as "QListView*"] -> bool as "bool" { return self -> uniformItemSizes () ; })
    }
    pub fn setUniformItemSizes(self, value: bool) {
        let value = cpp ! (unsafe [self as "QListView*" , value as "bool"] -> bool as "bool" { self -> setUniformItemSizes (value) ; return value ; });
    }
    pub fn wordWrap(self) -> bool {
        cpp ! (unsafe [self as "QListView*"] -> bool as "bool" { return self -> wordWrap () ; })
    }
    pub fn setWordWrap(self, value: bool) {
        let value = cpp ! (unsafe [self as "QListView*" , value as "bool"] -> bool as "bool" { self -> setWordWrap (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QPushButtonPtr as "QPointer<QPushButton>");
cpp_class ! (pub unsafe struct QPushButton as "QPushButton*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QPushButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QPushButton>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQPushButton: IQAbstractButton {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QPushButton>>;
}
impl QPushButton {
    pub fn new() -> Option<QPushButtonPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QPushButtonPtr) as "std::tuple<bool, QPointer<QPushButton>>" { QPointer < QPushButton > btn = new QPushButton () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QPushButton> {
    pub fn autoDefault(self) -> bool {
        cpp ! (unsafe [self as "QPushButton*"] -> bool as "bool" { return self -> autoDefault () ; })
    }
    pub fn setAutoDefault(self, value: bool) {
        let value = cpp ! (unsafe [self as "QPushButton*" , value as "bool"] -> bool as "bool" { self -> setAutoDefault (value) ; return value ; });
    }
    pub fn isDefault(self) -> bool {
        cpp ! (unsafe [self as "QPushButton*"] -> bool as "bool" { return self -> isDefault () ; })
    }
    pub fn setDefault(self, value: bool) {
        let value = cpp ! (unsafe [self as "QPushButton*" , value as "bool"] -> bool as "bool" { self -> setDefault (value) ; return value ; });
    }
    pub fn isFlat(self) -> bool {
        cpp ! (unsafe [self as "QPushButton*"] -> bool as "bool" { return self -> isFlat () ; })
    }
    pub fn setFlat(self, value: bool) {
        let value = cpp ! (unsafe [self as "QPushButton*" , value as "bool"] -> bool as "bool" { self -> setFlat (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QTimeEditPtr as "QPointer<QTimeEdit>");
cpp_class ! (pub unsafe struct QTimeEdit as "QTimeEdit*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QTimeEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTimeEdit>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQTimeEdit: IQDateTimeEdit {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QTimeEdit>>;
}
impl QTimeEdit {
    pub fn new() -> Option<QTimeEditPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QTimeEditPtr) as "std::tuple<bool, QPointer<QTimeEdit>>" { QPointer < QTimeEdit > btn = new QTimeEdit () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
cpp_class ! (pub unsafe struct QMessageBoxPtr as "QPointer<QMessageBox>");
cpp_class ! (pub unsafe struct QMessageBox as "QMessageBox*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QMessageBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMessageBox>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQMessageBox: IQDialog {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QMessageBox>>;
}
impl QMessageBox {
    pub fn new() -> Option<QMessageBoxPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QMessageBoxPtr) as "std::tuple<bool, QPointer<QMessageBox>>" { QPointer < QMessageBox > btn = new QMessageBox () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QMessageBox> {
    pub fn detailedText(self) -> QString {
        cpp ! (unsafe [self as "QMessageBox*"] -> QString as "QString" { return QString (self -> detailedText ()) ; })
    }
    pub fn setDetailedText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QMessageBox*" , value as "QString"] -> QString as "QString" { self -> setDetailedText (value) ; return value ; });
    }
    pub fn informativeText(self) -> QString {
        cpp ! (unsafe [self as "QMessageBox*"] -> QString as "QString" { return QString (self -> informativeText ()) ; })
    }
    pub fn setInformativeText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QMessageBox*" , value as "QString"] -> QString as "QString" { self -> setInformativeText (value) ; return value ; });
    }
    pub fn text(self) -> QString {
        cpp ! (unsafe [self as "QMessageBox*"] -> QString as "QString" { return QString (self -> text ()) ; })
    }
    pub fn setText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QMessageBox*" , value as "QString"] -> QString as "QString" { self -> setText (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QTreeViewPtr as "QPointer<QTreeView>");
cpp_class ! (pub unsafe struct QTreeView as "QTreeView*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QTreeViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeView>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQTreeView: IQAbstractItemView {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QTreeView>>;
}
impl QTreeView {
    pub fn new() -> Option<QTreeViewPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QTreeViewPtr) as "std::tuple<bool, QPointer<QTreeView>>" { QPointer < QTreeView > btn = new QTreeView () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QTreeView> {
    pub fn allColumnsShowFocus(self) -> bool {
        cpp ! (unsafe [self as "QTreeView*"] -> bool as "bool" { return self -> allColumnsShowFocus () ; })
    }
    pub fn setAllColumnsShowFocus(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTreeView*" , value as "bool"] -> bool as "bool" { self -> setAllColumnsShowFocus (value) ; return value ; });
    }
    pub fn isAnimated(self) -> bool {
        cpp ! (unsafe [self as "QTreeView*"] -> bool as "bool" { return self -> isAnimated () ; })
    }
    pub fn setAnimated(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTreeView*" , value as "bool"] -> bool as "bool" { self -> setAnimated (value) ; return value ; });
    }
    pub fn autoExpandDelay(self) -> int {
        cpp ! (unsafe [self as "QTreeView*"] -> int as "int" { return self -> autoExpandDelay () ; })
    }
    pub fn setAutoExpandDelay(self, value: int) {
        let value = cpp ! (unsafe [self as "QTreeView*" , value as "int"] -> int as "int" { self -> setAutoExpandDelay (value) ; return value ; });
    }
    pub fn expandsOnDoubleClick(self) -> bool {
        cpp ! (unsafe [self as "QTreeView*"] -> bool as "bool" { return self -> expandsOnDoubleClick () ; })
    }
    pub fn setExpandsOnDoubleClick(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTreeView*" , value as "bool"] -> bool as "bool" { self -> setExpandsOnDoubleClick (value) ; return value ; });
    }
    pub fn isHeaderHidden(self) -> bool {
        cpp ! (unsafe [self as "QTreeView*"] -> bool as "bool" { return self -> isHeaderHidden () ; })
    }
    pub fn setHeaderHidden(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTreeView*" , value as "bool"] -> bool as "bool" { self -> setHeaderHidden (value) ; return value ; });
    }
    pub fn indentation(self) -> int {
        cpp ! (unsafe [self as "QTreeView*"] -> int as "int" { return self -> indentation () ; })
    }
    pub fn setIndentation(self, value: int) {
        let value = cpp ! (unsafe [self as "QTreeView*" , value as "int"] -> int as "int" { self -> setIndentation (value) ; return value ; });
    }
    pub fn itemsExpandable(self) -> bool {
        cpp ! (unsafe [self as "QTreeView*"] -> bool as "bool" { return self -> itemsExpandable () ; })
    }
    pub fn setItemsExpandable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTreeView*" , value as "bool"] -> bool as "bool" { self -> setItemsExpandable (value) ; return value ; });
    }
    pub fn rootIsDecorated(self) -> bool {
        cpp ! (unsafe [self as "QTreeView*"] -> bool as "bool" { return self -> rootIsDecorated () ; })
    }
    pub fn setRootIsDecorated(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTreeView*" , value as "bool"] -> bool as "bool" { self -> setRootIsDecorated (value) ; return value ; });
    }
    pub fn isSortingEnabled(self) -> bool {
        cpp ! (unsafe [self as "QTreeView*"] -> bool as "bool" { return self -> isSortingEnabled () ; })
    }
    pub fn setSortingEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTreeView*" , value as "bool"] -> bool as "bool" { self -> setSortingEnabled (value) ; return value ; });
    }
    pub fn uniformRowHeights(self) -> bool {
        cpp ! (unsafe [self as "QTreeView*"] -> bool as "bool" { return self -> uniformRowHeights () ; })
    }
    pub fn setUniformRowHeights(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTreeView*" , value as "bool"] -> bool as "bool" { self -> setUniformRowHeights (value) ; return value ; });
    }
    pub fn wordWrap(self) -> bool {
        cpp ! (unsafe [self as "QTreeView*"] -> bool as "bool" { return self -> wordWrap () ; })
    }
    pub fn setWordWrap(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTreeView*" , value as "bool"] -> bool as "bool" { self -> setWordWrap (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QAbstractSpinBoxPtr as "QPointer<QAbstractSpinBox>");
cpp_class ! (pub unsafe struct QAbstractSpinBox as "QAbstractSpinBox*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QAbstractSpinBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractSpinBox>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQAbstractSpinBox: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractSpinBox>>;
}
impl<'a> QRef<'a, QAbstractSpinBox> {
    pub fn isAccelerated(self) -> bool {
        cpp ! (unsafe [self as "QAbstractSpinBox*"] -> bool as "bool" { return self -> isAccelerated () ; })
    }
    pub fn setAccelerated(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractSpinBox*" , value as "bool"] -> bool as "bool" { self -> setAccelerated (value) ; return value ; });
    }
    pub fn hasFrame(self) -> bool {
        cpp ! (unsafe [self as "QAbstractSpinBox*"] -> bool as "bool" { return self -> hasFrame () ; })
    }
    pub fn setFrame(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractSpinBox*" , value as "bool"] -> bool as "bool" { self -> setFrame (value) ; return value ; });
    }
    pub fn keyboardTracking(self) -> bool {
        cpp ! (unsafe [self as "QAbstractSpinBox*"] -> bool as "bool" { return self -> keyboardTracking () ; })
    }
    pub fn setKeyboardTracking(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractSpinBox*" , value as "bool"] -> bool as "bool" { self -> setKeyboardTracking (value) ; return value ; });
    }
    pub fn isReadOnly(self) -> bool {
        cpp ! (unsafe [self as "QAbstractSpinBox*"] -> bool as "bool" { return self -> isReadOnly () ; })
    }
    pub fn setReadOnly(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractSpinBox*" , value as "bool"] -> bool as "bool" { self -> setReadOnly (value) ; return value ; });
    }
    pub fn isGroupSeparatorShown(self) -> bool {
        cpp ! (unsafe [self as "QAbstractSpinBox*"] -> bool as "bool" { return self -> isGroupSeparatorShown () ; })
    }
    pub fn setGroupSeparatorShown(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractSpinBox*" , value as "bool"] -> bool as "bool" { self -> setGroupSeparatorShown (value) ; return value ; });
    }
    pub fn specialValueText(self) -> QString {
        cpp ! (unsafe [self as "QAbstractSpinBox*"] -> QString as "QString" { return QString (self -> specialValueText ()) ; })
    }
    pub fn setSpecialValueText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QAbstractSpinBox*" , value as "QString"] -> QString as "QString" { self -> setSpecialValueText (value) ; return value ; });
    }
    pub fn wrapping(self) -> bool {
        cpp ! (unsafe [self as "QAbstractSpinBox*"] -> bool as "bool" { return self -> wrapping () ; })
    }
    pub fn setWrapping(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractSpinBox*" , value as "bool"] -> bool as "bool" { self -> setWrapping (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QFileDialogPtr as "QPointer<QFileDialog>");
cpp_class ! (pub unsafe struct QFileDialog as "QFileDialog*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QFileDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFileDialog>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQFileDialog: IQDialog {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFileDialog>>;
}
impl QFileDialog {
    pub fn new() -> Option<QFileDialogPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QFileDialogPtr) as "std::tuple<bool, QPointer<QFileDialog>>" { QPointer < QFileDialog > btn = new QFileDialog () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QFileDialog> {
    pub fn defaultSuffix(self) -> QString {
        cpp ! (unsafe [self as "QFileDialog*"] -> QString as "QString" { return QString (self -> defaultSuffix ()) ; })
    }
    pub fn setDefaultSuffix(self, value: QString) {
        let value = cpp ! (unsafe [self as "QFileDialog*" , value as "QString"] -> QString as "QString" { self -> setDefaultSuffix (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QAbstractSliderPtr as "QPointer<QAbstractSlider>");
cpp_class ! (pub unsafe struct QAbstractSlider as "QAbstractSlider*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QAbstractSliderPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractSlider>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQAbstractSlider: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractSlider>>;
}
impl<'a> QRef<'a, QAbstractSlider> {
    pub fn invertedAppearance(self) -> bool {
        cpp ! (unsafe [self as "QAbstractSlider*"] -> bool as "bool" { return self -> invertedAppearance () ; })
    }
    pub fn setInvertedAppearance(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractSlider*" , value as "bool"] -> bool as "bool" { self -> setInvertedAppearance (value) ; return value ; });
    }
    pub fn invertedControls(self) -> bool {
        cpp ! (unsafe [self as "QAbstractSlider*"] -> bool as "bool" { return self -> invertedControls () ; })
    }
    pub fn setInvertedControls(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractSlider*" , value as "bool"] -> bool as "bool" { self -> setInvertedControls (value) ; return value ; });
    }
    pub fn maximum(self) -> int {
        cpp ! (unsafe [self as "QAbstractSlider*"] -> int as "int" { return self -> maximum () ; })
    }
    pub fn setMaximum(self, value: int) {
        let value = cpp ! (unsafe [self as "QAbstractSlider*" , value as "int"] -> int as "int" { self -> setMaximum (value) ; return value ; });
    }
    pub fn minimum(self) -> int {
        cpp ! (unsafe [self as "QAbstractSlider*"] -> int as "int" { return self -> minimum () ; })
    }
    pub fn setMinimum(self, value: int) {
        let value = cpp ! (unsafe [self as "QAbstractSlider*" , value as "int"] -> int as "int" { self -> setMinimum (value) ; return value ; });
    }
    pub fn pageStep(self) -> int {
        cpp ! (unsafe [self as "QAbstractSlider*"] -> int as "int" { return self -> pageStep () ; })
    }
    pub fn setPageStep(self, value: int) {
        let value = cpp ! (unsafe [self as "QAbstractSlider*" , value as "int"] -> int as "int" { self -> setPageStep (value) ; return value ; });
    }
    pub fn singleStep(self) -> int {
        cpp ! (unsafe [self as "QAbstractSlider*"] -> int as "int" { return self -> singleStep () ; })
    }
    pub fn setSingleStep(self, value: int) {
        let value = cpp ! (unsafe [self as "QAbstractSlider*" , value as "int"] -> int as "int" { self -> setSingleStep (value) ; return value ; });
    }
    pub fn isSliderDown(self) -> bool {
        cpp ! (unsafe [self as "QAbstractSlider*"] -> bool as "bool" { return self -> isSliderDown () ; })
    }
    pub fn setSliderDown(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractSlider*" , value as "bool"] -> bool as "bool" { self -> setSliderDown (value) ; return value ; });
    }
    pub fn sliderPosition(self) -> int {
        cpp ! (unsafe [self as "QAbstractSlider*"] -> int as "int" { return self -> sliderPosition () ; })
    }
    pub fn setSliderPosition(self, value: int) {
        let value = cpp ! (unsafe [self as "QAbstractSlider*" , value as "int"] -> int as "int" { self -> setSliderPosition (value) ; return value ; });
    }
    pub fn hasTracking(self) -> bool {
        cpp ! (unsafe [self as "QAbstractSlider*"] -> bool as "bool" { return self -> hasTracking () ; })
    }
    pub fn setTracking(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractSlider*" , value as "bool"] -> bool as "bool" { self -> setTracking (value) ; return value ; });
    }
    pub fn value(self) -> int {
        cpp ! (unsafe [self as "QAbstractSlider*"] -> int as "int" { return self -> value () ; })
    }
    pub fn setValue(self, value: int) {
        let value = cpp ! (unsafe [self as "QAbstractSlider*" , value as "int"] -> int as "int" { self -> setValue (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QToolBoxPtr as "QPointer<QToolBox>");
cpp_class ! (pub unsafe struct QToolBox as "QToolBox*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QToolBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolBox>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQToolBox: IQFrame {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QToolBox>>;
}
impl QToolBox {
    pub fn new() -> Option<QToolBoxPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QToolBoxPtr) as "std::tuple<bool, QPointer<QToolBox>>" { QPointer < QToolBox > btn = new QToolBox () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QToolBox> {
    pub fn currentIndex(self) -> int {
        cpp ! (unsafe [self as "QToolBox*"] -> int as "int" { return self -> currentIndex () ; })
    }
    pub fn setCurrentIndex(self, value: int) {
        let value = cpp ! (unsafe [self as "QToolBox*" , value as "int"] -> int as "int" { self -> setCurrentIndex (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QProgressBarPtr as "QPointer<QProgressBar>");
cpp_class ! (pub unsafe struct QProgressBar as "QProgressBar*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QProgressBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QProgressBar>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQProgressBar: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QProgressBar>>;
}
impl QProgressBar {
    pub fn new() -> Option<QProgressBarPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QProgressBarPtr) as "std::tuple<bool, QPointer<QProgressBar>>" { QPointer < QProgressBar > btn = new QProgressBar () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QProgressBar> {
    pub fn format(self) -> QString {
        cpp ! (unsafe [self as "QProgressBar*"] -> QString as "QString" { return QString (self -> format ()) ; })
    }
    pub fn setFormat(self, value: QString) {
        let value = cpp ! (unsafe [self as "QProgressBar*" , value as "QString"] -> QString as "QString" { self -> setFormat (value) ; return value ; });
    }
    pub fn invertedAppearance(self) -> bool {
        cpp ! (unsafe [self as "QProgressBar*"] -> bool as "bool" { return self -> invertedAppearance () ; })
    }
    pub fn setInvertedAppearance(self, value: bool) {
        let value = cpp ! (unsafe [self as "QProgressBar*" , value as "bool"] -> bool as "bool" { self -> setInvertedAppearance (value) ; return value ; });
    }
    pub fn maximum(self) -> int {
        cpp ! (unsafe [self as "QProgressBar*"] -> int as "int" { return self -> maximum () ; })
    }
    pub fn setMaximum(self, value: int) {
        let value = cpp ! (unsafe [self as "QProgressBar*" , value as "int"] -> int as "int" { self -> setMaximum (value) ; return value ; });
    }
    pub fn minimum(self) -> int {
        cpp ! (unsafe [self as "QProgressBar*"] -> int as "int" { return self -> minimum () ; })
    }
    pub fn setMinimum(self, value: int) {
        let value = cpp ! (unsafe [self as "QProgressBar*" , value as "int"] -> int as "int" { self -> setMinimum (value) ; return value ; });
    }
    pub fn isTextVisible(self) -> bool {
        cpp ! (unsafe [self as "QProgressBar*"] -> bool as "bool" { return self -> isTextVisible () ; })
    }
    pub fn setTextVisible(self, value: bool) {
        let value = cpp ! (unsafe [self as "QProgressBar*" , value as "bool"] -> bool as "bool" { self -> setTextVisible (value) ; return value ; });
    }
    pub fn value(self) -> int {
        cpp ! (unsafe [self as "QProgressBar*"] -> int as "int" { return self -> value () ; })
    }
    pub fn setValue(self, value: int) {
        let value = cpp ! (unsafe [self as "QProgressBar*" , value as "int"] -> int as "int" { self -> setValue (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QFramePtr as "QPointer<QFrame>");
cpp_class ! (pub unsafe struct QFrame as "QFrame*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QFramePtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFrame>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQFrame: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>>;
}
impl QFrame {
    pub fn new() -> Option<QFramePtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QFramePtr) as "std::tuple<bool, QPointer<QFrame>>" { QPointer < QFrame > btn = new QFrame () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QFrame> {
    pub fn lineWidth(self) -> int {
        cpp ! (unsafe [self as "QFrame*"] -> int as "int" { return self -> lineWidth () ; })
    }
    pub fn setLineWidth(self, value: int) {
        let value = cpp ! (unsafe [self as "QFrame*" , value as "int"] -> int as "int" { self -> setLineWidth (value) ; return value ; });
    }
    pub fn midLineWidth(self) -> int {
        cpp ! (unsafe [self as "QFrame*"] -> int as "int" { return self -> midLineWidth () ; })
    }
    pub fn setMidLineWidth(self, value: int) {
        let value = cpp ! (unsafe [self as "QFrame*" , value as "int"] -> int as "int" { self -> setMidLineWidth (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QTabBarPtr as "QPointer<QTabBar>");
cpp_class ! (pub unsafe struct QTabBar as "QTabBar*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QTabBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTabBar>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQTabBar: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QTabBar>>;
}
impl QTabBar {
    pub fn new() -> Option<QTabBarPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QTabBarPtr) as "std::tuple<bool, QPointer<QTabBar>>" { QPointer < QTabBar > btn = new QTabBar () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QTabBar> {
    pub fn autoHide(self) -> bool {
        cpp ! (unsafe [self as "QTabBar*"] -> bool as "bool" { return self -> autoHide () ; })
    }
    pub fn setAutoHide(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabBar*" , value as "bool"] -> bool as "bool" { self -> setAutoHide (value) ; return value ; });
    }
    pub fn changeCurrentOnDrag(self) -> bool {
        cpp ! (unsafe [self as "QTabBar*"] -> bool as "bool" { return self -> changeCurrentOnDrag () ; })
    }
    pub fn setChangeCurrentOnDrag(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabBar*" , value as "bool"] -> bool as "bool" { self -> setChangeCurrentOnDrag (value) ; return value ; });
    }
    pub fn currentIndex(self) -> int {
        cpp ! (unsafe [self as "QTabBar*"] -> int as "int" { return self -> currentIndex () ; })
    }
    pub fn setCurrentIndex(self, value: int) {
        let value = cpp ! (unsafe [self as "QTabBar*" , value as "int"] -> int as "int" { self -> setCurrentIndex (value) ; return value ; });
    }
    pub fn documentMode(self) -> bool {
        cpp ! (unsafe [self as "QTabBar*"] -> bool as "bool" { return self -> documentMode () ; })
    }
    pub fn setDocumentMode(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabBar*" , value as "bool"] -> bool as "bool" { self -> setDocumentMode (value) ; return value ; });
    }
    pub fn drawBase(self) -> bool {
        cpp ! (unsafe [self as "QTabBar*"] -> bool as "bool" { return self -> drawBase () ; })
    }
    pub fn setDrawBase(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabBar*" , value as "bool"] -> bool as "bool" { self -> setDrawBase (value) ; return value ; });
    }
    pub fn expanding(self) -> bool {
        cpp ! (unsafe [self as "QTabBar*"] -> bool as "bool" { return self -> expanding () ; })
    }
    pub fn setExpanding(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabBar*" , value as "bool"] -> bool as "bool" { self -> setExpanding (value) ; return value ; });
    }
    pub fn iconSize(self) -> QSize {
        cpp ! (unsafe [self as "QTabBar*"] -> QSize as "QSize" { return QSize (self -> iconSize ()) ; })
    }
    pub fn setIconSize(self, value: QSize) {
        let value = cpp ! (unsafe [self as "QTabBar*" , value as "QSize"] -> QSize as "QSize" { self -> setIconSize (value) ; return value ; });
    }
    pub fn isMovable(self) -> bool {
        cpp ! (unsafe [self as "QTabBar*"] -> bool as "bool" { return self -> isMovable () ; })
    }
    pub fn setMovable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabBar*" , value as "bool"] -> bool as "bool" { self -> setMovable (value) ; return value ; });
    }
    pub fn tabsClosable(self) -> bool {
        cpp ! (unsafe [self as "QTabBar*"] -> bool as "bool" { return self -> tabsClosable () ; })
    }
    pub fn setTabsClosable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabBar*" , value as "bool"] -> bool as "bool" { self -> setTabsClosable (value) ; return value ; });
    }
    pub fn usesScrollButtons(self) -> bool {
        cpp ! (unsafe [self as "QTabBar*"] -> bool as "bool" { return self -> usesScrollButtons () ; })
    }
    pub fn setUsesScrollButtons(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabBar*" , value as "bool"] -> bool as "bool" { self -> setUsesScrollButtons (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QTableWidgetPtr as "QPointer<QTableWidget>");
cpp_class ! (pub unsafe struct QTableWidget as "QTableWidget*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QTableWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableWidget>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQTableWidget: IQTableView {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QTableWidget>>;
}
impl QTableWidget {
    pub fn new() -> Option<QTableWidgetPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QTableWidgetPtr) as "std::tuple<bool, QPointer<QTableWidget>>" { QPointer < QTableWidget > btn = new QTableWidget () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QTableWidget> {
    pub fn columnCount(self) -> int {
        cpp ! (unsafe [self as "QTableWidget*"] -> int as "int" { return self -> columnCount () ; })
    }
    pub fn setColumnCount(self, value: int) {
        let value = cpp ! (unsafe [self as "QTableWidget*" , value as "int"] -> int as "int" { self -> setColumnCount (value) ; return value ; });
    }
    pub fn rowCount(self) -> int {
        cpp ! (unsafe [self as "QTableWidget*"] -> int as "int" { return self -> rowCount () ; })
    }
    pub fn setRowCount(self, value: int) {
        let value = cpp ! (unsafe [self as "QTableWidget*" , value as "int"] -> int as "int" { self -> setRowCount (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QDockWidgetPtr as "QPointer<QDockWidget>");
cpp_class ! (pub unsafe struct QDockWidget as "QDockWidget*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QDockWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDockWidget>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQDockWidget: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDockWidget>>;
}
impl QDockWidget {
    pub fn new() -> Option<QDockWidgetPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QDockWidgetPtr) as "std::tuple<bool, QPointer<QDockWidget>>" { QPointer < QDockWidget > btn = new QDockWidget () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QDockWidget> {
    pub fn isFloating(self) -> bool {
        cpp ! (unsafe [self as "QDockWidget*"] -> bool as "bool" { return self -> isFloating () ; })
    }
    pub fn setFloating(self, value: bool) {
        let value = cpp ! (unsafe [self as "QDockWidget*" , value as "bool"] -> bool as "bool" { self -> setFloating (value) ; return value ; });
    }
    pub fn windowTitle(self) -> QString {
        cpp ! (unsafe [self as "QDockWidget*"] -> QString as "QString" { return QString (self -> windowTitle ()) ; })
    }
    pub fn setWindowTitle(self, value: QString) {
        let value = cpp ! (unsafe [self as "QDockWidget*" , value as "QString"] -> QString as "QString" { self -> setWindowTitle (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QToolButtonPtr as "QPointer<QToolButton>");
cpp_class ! (pub unsafe struct QToolButton as "QToolButton*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QToolButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolButton>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQToolButton: IQAbstractButton {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QToolButton>>;
}
impl QToolButton {
    pub fn new() -> Option<QToolButtonPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QToolButtonPtr) as "std::tuple<bool, QPointer<QToolButton>>" { QPointer < QToolButton > btn = new QToolButton () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QToolButton> {
    pub fn autoRaise(self) -> bool {
        cpp ! (unsafe [self as "QToolButton*"] -> bool as "bool" { return self -> autoRaise () ; })
    }
    pub fn setAutoRaise(self, value: bool) {
        let value = cpp ! (unsafe [self as "QToolButton*" , value as "bool"] -> bool as "bool" { self -> setAutoRaise (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QStatusBarPtr as "QPointer<QStatusBar>");
cpp_class ! (pub unsafe struct QStatusBar as "QStatusBar*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QStatusBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QStatusBar>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQStatusBar: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QStatusBar>>;
}
impl QStatusBar {
    pub fn new() -> Option<QStatusBarPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QStatusBarPtr) as "std::tuple<bool, QPointer<QStatusBar>>" { QPointer < QStatusBar > btn = new QStatusBar () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QStatusBar> {
    pub fn isSizeGripEnabled(self) -> bool {
        cpp ! (unsafe [self as "QStatusBar*"] -> bool as "bool" { return self -> isSizeGripEnabled () ; })
    }
    pub fn setSizeGripEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QStatusBar*" , value as "bool"] -> bool as "bool" { self -> setSizeGripEnabled (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QUndoViewPtr as "QPointer<QUndoView>");
cpp_class ! (pub unsafe struct QUndoView as "QUndoView*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QUndoViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QUndoView>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQUndoView: IQListView {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QUndoView>>;
}
impl QUndoView {
    pub fn new() -> Option<QUndoViewPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QUndoViewPtr) as "std::tuple<bool, QPointer<QUndoView>>" { QPointer < QUndoView > btn = new QUndoView () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QUndoView> {
    pub fn emptyLabel(self) -> QString {
        cpp ! (unsafe [self as "QUndoView*"] -> QString as "QString" { return QString (self -> emptyLabel ()) ; })
    }
    pub fn setEmptyLabel(self, value: QString) {
        let value = cpp ! (unsafe [self as "QUndoView*" , value as "QString"] -> QString as "QString" { self -> setEmptyLabel (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QTreeWidgetPtr as "QPointer<QTreeWidget>");
cpp_class ! (pub unsafe struct QTreeWidget as "QTreeWidget*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QTreeWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeWidget>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQTreeWidget: IQTreeView {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QTreeWidget>>;
}
impl QTreeWidget {
    pub fn new() -> Option<QTreeWidgetPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QTreeWidgetPtr) as "std::tuple<bool, QPointer<QTreeWidget>>" { QPointer < QTreeWidget > btn = new QTreeWidget () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QTreeWidget> {
    pub fn columnCount(self) -> int {
        cpp ! (unsafe [self as "QTreeWidget*"] -> int as "int" { return self -> columnCount () ; })
    }
    pub fn setColumnCount(self, value: int) {
        let value = cpp ! (unsafe [self as "QTreeWidget*" , value as "int"] -> int as "int" { self -> setColumnCount (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QMainWindowPtr as "QPointer<QMainWindow>");
cpp_class ! (pub unsafe struct QMainWindow as "QMainWindow*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QMainWindowPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMainWindow>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQMainWindow: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QMainWindow>>;
}
impl QMainWindow {
    pub fn new() -> Option<QMainWindowPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QMainWindowPtr) as "std::tuple<bool, QPointer<QMainWindow>>" { QPointer < QMainWindow > btn = new QMainWindow () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QMainWindow> {
    pub fn isAnimated(self) -> bool {
        cpp ! (unsafe [self as "QMainWindow*"] -> bool as "bool" { return self -> isAnimated () ; })
    }
    pub fn setAnimated(self, value: bool) {
        let value = cpp ! (unsafe [self as "QMainWindow*" , value as "bool"] -> bool as "bool" { self -> setAnimated (value) ; return value ; });
    }
    pub fn isDockNestingEnabled(self) -> bool {
        cpp ! (unsafe [self as "QMainWindow*"] -> bool as "bool" { return self -> isDockNestingEnabled () ; })
    }
    pub fn setDockNestingEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QMainWindow*" , value as "bool"] -> bool as "bool" { self -> setDockNestingEnabled (value) ; return value ; });
    }
    pub fn documentMode(self) -> bool {
        cpp ! (unsafe [self as "QMainWindow*"] -> bool as "bool" { return self -> documentMode () ; })
    }
    pub fn setDocumentMode(self, value: bool) {
        let value = cpp ! (unsafe [self as "QMainWindow*" , value as "bool"] -> bool as "bool" { self -> setDocumentMode (value) ; return value ; });
    }
    pub fn iconSize(self) -> QSize {
        cpp ! (unsafe [self as "QMainWindow*"] -> QSize as "QSize" { return QSize (self -> iconSize ()) ; })
    }
    pub fn setIconSize(self, value: QSize) {
        let value = cpp ! (unsafe [self as "QMainWindow*" , value as "QSize"] -> QSize as "QSize" { self -> setIconSize (value) ; return value ; });
    }
    pub fn unifiedTitleAndToolBarOnMac(self) -> bool {
        cpp ! (unsafe [self as "QMainWindow*"] -> bool as "bool" { return self -> unifiedTitleAndToolBarOnMac () ; })
    }
    pub fn setUnifiedTitleAndToolBarOnMac(self, value: bool) {
        let value = cpp ! (unsafe [self as "QMainWindow*" , value as "bool"] -> bool as "bool" { self -> setUnifiedTitleAndToolBarOnMac (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QOpenGLWidgetPtr as "QPointer<QOpenGLWidget>");
cpp_class ! (pub unsafe struct QOpenGLWidget as "QOpenGLWidget*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QOpenGLWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QOpenGLWidget>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQOpenGLWidget: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QOpenGLWidget>>;
}
impl QOpenGLWidget {
    pub fn new() -> Option<QOpenGLWidgetPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QOpenGLWidgetPtr) as "std::tuple<bool, QPointer<QOpenGLWidget>>" { QPointer < QOpenGLWidget > btn = new QOpenGLWidget () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
cpp_class ! (pub unsafe struct QWizardPagePtr as "QPointer<QWizardPage>");
cpp_class ! (pub unsafe struct QWizardPage as "QWizardPage*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QWizardPagePtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QWizardPage>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQWizardPage: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWizardPage>>;
}
impl QWizardPage {
    pub fn new() -> Option<QWizardPagePtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QWizardPagePtr) as "std::tuple<bool, QPointer<QWizardPage>>" { QPointer < QWizardPage > btn = new QWizardPage () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QWizardPage> {
    pub fn subTitle(self) -> QString {
        cpp ! (unsafe [self as "QWizardPage*"] -> QString as "QString" { return QString (self -> subTitle ()) ; })
    }
    pub fn setSubTitle(self, value: QString) {
        let value = cpp ! (unsafe [self as "QWizardPage*" , value as "QString"] -> QString as "QString" { self -> setSubTitle (value) ; return value ; });
    }
    pub fn title(self) -> QString {
        cpp ! (unsafe [self as "QWizardPage*"] -> QString as "QString" { return QString (self -> title ()) ; })
    }
    pub fn setTitle(self, value: QString) {
        let value = cpp ! (unsafe [self as "QWizardPage*" , value as "QString"] -> QString as "QString" { self -> setTitle (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QTextEditPtr as "QPointer<QTextEdit>");
cpp_class ! (pub unsafe struct QTextEdit as "QTextEdit*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QTextEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextEdit>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQTextEdit: IQAbstractScrollArea {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QTextEdit>>;
}
impl QTextEdit {
    pub fn new() -> Option<QTextEditPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QTextEditPtr) as "std::tuple<bool, QPointer<QTextEdit>>" { QPointer < QTextEdit > btn = new QTextEdit () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QTextEdit> {
    pub fn acceptRichText(self) -> bool {
        cpp ! (unsafe [self as "QTextEdit*"] -> bool as "bool" { return self -> acceptRichText () ; })
    }
    pub fn setAcceptRichText(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "bool"] -> bool as "bool" { self -> setAcceptRichText (value) ; return value ; });
    }
    pub fn cursorWidth(self) -> int {
        cpp ! (unsafe [self as "QTextEdit*"] -> int as "int" { return self -> cursorWidth () ; })
    }
    pub fn setCursorWidth(self, value: int) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "int"] -> int as "int" { self -> setCursorWidth (value) ; return value ; });
    }
    pub fn documentTitle(self) -> QString {
        cpp ! (unsafe [self as "QTextEdit*"] -> QString as "QString" { return QString (self -> documentTitle ()) ; })
    }
    pub fn setDocumentTitle(self, value: QString) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "QString"] -> QString as "QString" { self -> setDocumentTitle (value) ; return value ; });
    }
    pub fn toHtml(self) -> QString {
        cpp ! (unsafe [self as "QTextEdit*"] -> QString as "QString" { return QString (self -> toHtml ()) ; })
    }
    pub fn setHtml(self, value: QString) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "QString"] -> QString as "QString" { self -> setHtml (value) ; return value ; });
    }
    pub fn lineWrapColumnOrWidth(self) -> int {
        cpp ! (unsafe [self as "QTextEdit*"] -> int as "int" { return self -> lineWrapColumnOrWidth () ; })
    }
    pub fn setLineWrapColumnOrWidth(self, value: int) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "int"] -> int as "int" { self -> setLineWrapColumnOrWidth (value) ; return value ; });
    }
    pub fn toMarkdown(self) -> QString {
        cpp ! (unsafe [self as "QTextEdit*"] -> QString as "QString" { return QString (self -> toMarkdown ()) ; })
    }
    pub fn setMarkdown(self, value: QString) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "QString"] -> QString as "QString" { self -> setMarkdown (value) ; return value ; });
    }
    pub fn overwriteMode(self) -> bool {
        cpp ! (unsafe [self as "QTextEdit*"] -> bool as "bool" { return self -> overwriteMode () ; })
    }
    pub fn setOverwriteMode(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "bool"] -> bool as "bool" { self -> setOverwriteMode (value) ; return value ; });
    }
    pub fn placeholderText(self) -> QString {
        cpp ! (unsafe [self as "QTextEdit*"] -> QString as "QString" { return QString (self -> placeholderText ()) ; })
    }
    pub fn setPlaceholderText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "QString"] -> QString as "QString" { self -> setPlaceholderText (value) ; return value ; });
    }
    pub fn toPlainText(self) -> QString {
        cpp ! (unsafe [self as "QTextEdit*"] -> QString as "QString" { return QString (self -> toPlainText ()) ; })
    }
    pub fn setPlainText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "QString"] -> QString as "QString" { self -> setPlainText (value) ; return value ; });
    }
    pub fn isReadOnly(self) -> bool {
        cpp ! (unsafe [self as "QTextEdit*"] -> bool as "bool" { return self -> isReadOnly () ; })
    }
    pub fn setReadOnly(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "bool"] -> bool as "bool" { self -> setReadOnly (value) ; return value ; });
    }
    pub fn tabChangesFocus(self) -> bool {
        cpp ! (unsafe [self as "QTextEdit*"] -> bool as "bool" { return self -> tabChangesFocus () ; })
    }
    pub fn setTabChangesFocus(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "bool"] -> bool as "bool" { self -> setTabChangesFocus (value) ; return value ; });
    }
    pub fn tabStopDistance(self) -> qreal {
        cpp ! (unsafe [self as "QTextEdit*"] -> qreal as "qreal" { return self -> tabStopDistance () ; })
    }
    pub fn setTabStopDistance(self, value: qreal) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "qreal"] -> qreal as "qreal" { self -> setTabStopDistance (value) ; return value ; });
    }
    pub fn isUndoRedoEnabled(self) -> bool {
        cpp ! (unsafe [self as "QTextEdit*"] -> bool as "bool" { return self -> isUndoRedoEnabled () ; })
    }
    pub fn setUndoRedoEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTextEdit*" , value as "bool"] -> bool as "bool" { self -> setUndoRedoEnabled (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QMenuBarPtr as "QPointer<QMenuBar>");
cpp_class ! (pub unsafe struct QMenuBar as "QMenuBar*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QMenuBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMenuBar>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQMenuBar: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QMenuBar>>;
}
impl QMenuBar {
    pub fn new() -> Option<QMenuBarPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QMenuBarPtr) as "std::tuple<bool, QPointer<QMenuBar>>" { QPointer < QMenuBar > btn = new QMenuBar () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QMenuBar> {
    pub fn isDefaultUp(self) -> bool {
        cpp ! (unsafe [self as "QMenuBar*"] -> bool as "bool" { return self -> isDefaultUp () ; })
    }
    pub fn setDefaultUp(self, value: bool) {
        let value = cpp ! (unsafe [self as "QMenuBar*" , value as "bool"] -> bool as "bool" { self -> setDefaultUp (value) ; return value ; });
    }
    pub fn isNativeMenuBar(self) -> bool {
        cpp ! (unsafe [self as "QMenuBar*"] -> bool as "bool" { return self -> isNativeMenuBar () ; })
    }
    pub fn setNativeMenuBar(self, value: bool) {
        let value = cpp ! (unsafe [self as "QMenuBar*" , value as "bool"] -> bool as "bool" { self -> setNativeMenuBar (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QMdiAreaPtr as "QPointer<QMdiArea>");
cpp_class ! (pub unsafe struct QMdiArea as "QMdiArea*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QMdiAreaPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMdiArea>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQMdiArea: IQAbstractScrollArea {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QMdiArea>>;
}
impl QMdiArea {
    pub fn new() -> Option<QMdiAreaPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QMdiAreaPtr) as "std::tuple<bool, QPointer<QMdiArea>>" { QPointer < QMdiArea > btn = new QMdiArea () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QMdiArea> {
    pub fn documentMode(self) -> bool {
        cpp ! (unsafe [self as "QMdiArea*"] -> bool as "bool" { return self -> documentMode () ; })
    }
    pub fn setDocumentMode(self, value: bool) {
        let value = cpp ! (unsafe [self as "QMdiArea*" , value as "bool"] -> bool as "bool" { self -> setDocumentMode (value) ; return value ; });
    }
    pub fn tabsClosable(self) -> bool {
        cpp ! (unsafe [self as "QMdiArea*"] -> bool as "bool" { return self -> tabsClosable () ; })
    }
    pub fn setTabsClosable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QMdiArea*" , value as "bool"] -> bool as "bool" { self -> setTabsClosable (value) ; return value ; });
    }
    pub fn tabsMovable(self) -> bool {
        cpp ! (unsafe [self as "QMdiArea*"] -> bool as "bool" { return self -> tabsMovable () ; })
    }
    pub fn setTabsMovable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QMdiArea*" , value as "bool"] -> bool as "bool" { self -> setTabsMovable (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QSplitterPtr as "QPointer<QSplitter>");
cpp_class ! (pub unsafe struct QSplitter as "QSplitter*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QSplitterPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSplitter>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQSplitter: IQFrame {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QSplitter>>;
}
impl QSplitter {
    pub fn new() -> Option<QSplitterPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QSplitterPtr) as "std::tuple<bool, QPointer<QSplitter>>" { QPointer < QSplitter > btn = new QSplitter () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QSplitter> {
    pub fn childrenCollapsible(self) -> bool {
        cpp ! (unsafe [self as "QSplitter*"] -> bool as "bool" { return self -> childrenCollapsible () ; })
    }
    pub fn setChildrenCollapsible(self, value: bool) {
        let value = cpp ! (unsafe [self as "QSplitter*" , value as "bool"] -> bool as "bool" { self -> setChildrenCollapsible (value) ; return value ; });
    }
    pub fn handleWidth(self) -> int {
        cpp ! (unsafe [self as "QSplitter*"] -> int as "int" { return self -> handleWidth () ; })
    }
    pub fn setHandleWidth(self, value: int) {
        let value = cpp ! (unsafe [self as "QSplitter*" , value as "int"] -> int as "int" { self -> setHandleWidth (value) ; return value ; });
    }
    pub fn opaqueResize(self) -> bool {
        cpp ! (unsafe [self as "QSplitter*"] -> bool as "bool" { return self -> opaqueResize () ; })
    }
    pub fn setOpaqueResize(self, value: bool) {
        let value = cpp ! (unsafe [self as "QSplitter*" , value as "bool"] -> bool as "bool" { self -> setOpaqueResize (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QListWidgetPtr as "QPointer<QListWidget>");
cpp_class ! (pub unsafe struct QListWidget as "QListWidget*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QListWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QListWidget>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQListWidget: IQListView {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QListWidget>>;
}
impl QListWidget {
    pub fn new() -> Option<QListWidgetPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QListWidgetPtr) as "std::tuple<bool, QPointer<QListWidget>>" { QPointer < QListWidget > btn = new QListWidget () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QListWidget> {
    pub fn currentRow(self) -> int {
        cpp ! (unsafe [self as "QListWidget*"] -> int as "int" { return self -> currentRow () ; })
    }
    pub fn setCurrentRow(self, value: int) {
        let value = cpp ! (unsafe [self as "QListWidget*" , value as "int"] -> int as "int" { self -> setCurrentRow (value) ; return value ; });
    }
    pub fn isSortingEnabled(self) -> bool {
        cpp ! (unsafe [self as "QListWidget*"] -> bool as "bool" { return self -> isSortingEnabled () ; })
    }
    pub fn setSortingEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QListWidget*" , value as "bool"] -> bool as "bool" { self -> setSortingEnabled (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QMdiSubWindowPtr as "QPointer<QMdiSubWindow>");
cpp_class ! (pub unsafe struct QMdiSubWindow as "QMdiSubWindow*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QMdiSubWindowPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMdiSubWindow>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQMdiSubWindow: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QMdiSubWindow>>;
}
impl QMdiSubWindow {
    pub fn new() -> Option<QMdiSubWindowPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QMdiSubWindowPtr) as "std::tuple<bool, QPointer<QMdiSubWindow>>" { QPointer < QMdiSubWindow > btn = new QMdiSubWindow () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QMdiSubWindow> {
    pub fn keyboardPageStep(self) -> int {
        cpp ! (unsafe [self as "QMdiSubWindow*"] -> int as "int" { return self -> keyboardPageStep () ; })
    }
    pub fn setKeyboardPageStep(self, value: int) {
        let value = cpp ! (unsafe [self as "QMdiSubWindow*" , value as "int"] -> int as "int" { self -> setKeyboardPageStep (value) ; return value ; });
    }
    pub fn keyboardSingleStep(self) -> int {
        cpp ! (unsafe [self as "QMdiSubWindow*"] -> int as "int" { return self -> keyboardSingleStep () ; })
    }
    pub fn setKeyboardSingleStep(self, value: int) {
        let value = cpp ! (unsafe [self as "QMdiSubWindow*" , value as "int"] -> int as "int" { self -> setKeyboardSingleStep (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QHeaderViewPtr as "QPointer<QHeaderView>");
cpp_class ! (pub unsafe struct QHeaderView as "QHeaderView*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QHeaderViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QHeaderView>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQHeaderView: IQAbstractItemView {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QHeaderView>>;
}
impl<'a> QRef<'a, QHeaderView> {
    pub fn cascadingSectionResizes(self) -> bool {
        cpp ! (unsafe [self as "QHeaderView*"] -> bool as "bool" { return self -> cascadingSectionResizes () ; })
    }
    pub fn setCascadingSectionResizes(self, value: bool) {
        let value = cpp ! (unsafe [self as "QHeaderView*" , value as "bool"] -> bool as "bool" { self -> setCascadingSectionResizes (value) ; return value ; });
    }
    pub fn defaultSectionSize(self) -> int {
        cpp ! (unsafe [self as "QHeaderView*"] -> int as "int" { return self -> defaultSectionSize () ; })
    }
    pub fn setDefaultSectionSize(self, value: int) {
        let value = cpp ! (unsafe [self as "QHeaderView*" , value as "int"] -> int as "int" { self -> setDefaultSectionSize (value) ; return value ; });
    }
    pub fn isFirstSectionMovable(self) -> bool {
        cpp ! (unsafe [self as "QHeaderView*"] -> bool as "bool" { return self -> isFirstSectionMovable () ; })
    }
    pub fn setFirstSectionMovable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QHeaderView*" , value as "bool"] -> bool as "bool" { self -> setFirstSectionMovable (value) ; return value ; });
    }
    pub fn highlightSections(self) -> bool {
        cpp ! (unsafe [self as "QHeaderView*"] -> bool as "bool" { return self -> highlightSections () ; })
    }
    pub fn setHighlightSections(self, value: bool) {
        let value = cpp ! (unsafe [self as "QHeaderView*" , value as "bool"] -> bool as "bool" { self -> setHighlightSections (value) ; return value ; });
    }
    pub fn maximumSectionSize(self) -> int {
        cpp ! (unsafe [self as "QHeaderView*"] -> int as "int" { return self -> maximumSectionSize () ; })
    }
    pub fn setMaximumSectionSize(self, value: int) {
        let value = cpp ! (unsafe [self as "QHeaderView*" , value as "int"] -> int as "int" { self -> setMaximumSectionSize (value) ; return value ; });
    }
    pub fn minimumSectionSize(self) -> int {
        cpp ! (unsafe [self as "QHeaderView*"] -> int as "int" { return self -> minimumSectionSize () ; })
    }
    pub fn setMinimumSectionSize(self, value: int) {
        let value = cpp ! (unsafe [self as "QHeaderView*" , value as "int"] -> int as "int" { self -> setMinimumSectionSize (value) ; return value ; });
    }
    pub fn isSortIndicatorShown(self) -> bool {
        cpp ! (unsafe [self as "QHeaderView*"] -> bool as "bool" { return self -> isSortIndicatorShown () ; })
    }
    pub fn setSortIndicatorShown(self, value: bool) {
        let value = cpp ! (unsafe [self as "QHeaderView*" , value as "bool"] -> bool as "bool" { self -> setSortIndicatorShown (value) ; return value ; });
    }
    pub fn stretchLastSection(self) -> bool {
        cpp ! (unsafe [self as "QHeaderView*"] -> bool as "bool" { return self -> stretchLastSection () ; })
    }
    pub fn setStretchLastSection(self, value: bool) {
        let value = cpp ! (unsafe [self as "QHeaderView*" , value as "bool"] -> bool as "bool" { self -> setStretchLastSection (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QAbstractItemViewPtr as "QPointer<QAbstractItemView>");
cpp_class ! (pub unsafe struct QAbstractItemView as "QAbstractItemView*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QAbstractItemViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractItemView>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQAbstractItemView: IQAbstractScrollArea {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractItemView>>;
}
impl<'a> QRef<'a, QAbstractItemView> {
    pub fn alternatingRowColors(self) -> bool {
        cpp ! (unsafe [self as "QAbstractItemView*"] -> bool as "bool" { return self -> alternatingRowColors () ; })
    }
    pub fn setAlternatingRowColors(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractItemView*" , value as "bool"] -> bool as "bool" { self -> setAlternatingRowColors (value) ; return value ; });
    }
    pub fn hasAutoScroll(self) -> bool {
        cpp ! (unsafe [self as "QAbstractItemView*"] -> bool as "bool" { return self -> hasAutoScroll () ; })
    }
    pub fn setAutoScroll(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractItemView*" , value as "bool"] -> bool as "bool" { self -> setAutoScroll (value) ; return value ; });
    }
    pub fn autoScrollMargin(self) -> int {
        cpp ! (unsafe [self as "QAbstractItemView*"] -> int as "int" { return self -> autoScrollMargin () ; })
    }
    pub fn setAutoScrollMargin(self, value: int) {
        let value = cpp ! (unsafe [self as "QAbstractItemView*" , value as "int"] -> int as "int" { self -> setAutoScrollMargin (value) ; return value ; });
    }
    pub fn dragDropOverwriteMode(self) -> bool {
        cpp ! (unsafe [self as "QAbstractItemView*"] -> bool as "bool" { return self -> dragDropOverwriteMode () ; })
    }
    pub fn setDragDropOverwriteMode(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractItemView*" , value as "bool"] -> bool as "bool" { self -> setDragDropOverwriteMode (value) ; return value ; });
    }
    pub fn dragEnabled(self) -> bool {
        cpp ! (unsafe [self as "QAbstractItemView*"] -> bool as "bool" { return self -> dragEnabled () ; })
    }
    pub fn setDragEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractItemView*" , value as "bool"] -> bool as "bool" { self -> setDragEnabled (value) ; return value ; });
    }
    pub fn iconSize(self) -> QSize {
        cpp ! (unsafe [self as "QAbstractItemView*"] -> QSize as "QSize" { return QSize (self -> iconSize ()) ; })
    }
    pub fn setIconSize(self, value: QSize) {
        let value = cpp ! (unsafe [self as "QAbstractItemView*" , value as "QSize"] -> QSize as "QSize" { self -> setIconSize (value) ; return value ; });
    }
    pub fn showDropIndicator(self) -> bool {
        cpp ! (unsafe [self as "QAbstractItemView*"] -> bool as "bool" { return self -> showDropIndicator () ; })
    }
    pub fn setDropIndicatorShown(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractItemView*" , value as "bool"] -> bool as "bool" { self -> setDropIndicatorShown (value) ; return value ; });
    }
    pub fn tabKeyNavigation(self) -> bool {
        cpp ! (unsafe [self as "QAbstractItemView*"] -> bool as "bool" { return self -> tabKeyNavigation () ; })
    }
    pub fn setTabKeyNavigation(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractItemView*" , value as "bool"] -> bool as "bool" { self -> setTabKeyNavigation (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QDoubleSpinBoxPtr as "QPointer<QDoubleSpinBox>");
cpp_class ! (pub unsafe struct QDoubleSpinBox as "QDoubleSpinBox*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QDoubleSpinBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDoubleSpinBox>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQDoubleSpinBox: IQAbstractSpinBox {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDoubleSpinBox>>;
}
impl QDoubleSpinBox {
    pub fn new() -> Option<QDoubleSpinBoxPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QDoubleSpinBoxPtr) as "std::tuple<bool, QPointer<QDoubleSpinBox>>" { QPointer < QDoubleSpinBox > btn = new QDoubleSpinBox () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QDoubleSpinBox> {
    pub fn decimals(self) -> int {
        cpp ! (unsafe [self as "QDoubleSpinBox*"] -> int as "int" { return self -> decimals () ; })
    }
    pub fn setDecimals(self, value: int) {
        let value = cpp ! (unsafe [self as "QDoubleSpinBox*" , value as "int"] -> int as "int" { self -> setDecimals (value) ; return value ; });
    }
    pub fn maximum(self) -> double {
        cpp ! (unsafe [self as "QDoubleSpinBox*"] -> double as "double" { return self -> maximum () ; })
    }
    pub fn setMaximum(self, value: double) {
        let value = cpp ! (unsafe [self as "QDoubleSpinBox*" , value as "double"] -> double as "double" { self -> setMaximum (value) ; return value ; });
    }
    pub fn minimum(self) -> double {
        cpp ! (unsafe [self as "QDoubleSpinBox*"] -> double as "double" { return self -> minimum () ; })
    }
    pub fn setMinimum(self, value: double) {
        let value = cpp ! (unsafe [self as "QDoubleSpinBox*" , value as "double"] -> double as "double" { self -> setMinimum (value) ; return value ; });
    }
    pub fn prefix(self) -> QString {
        cpp ! (unsafe [self as "QDoubleSpinBox*"] -> QString as "QString" { return QString (self -> prefix ()) ; })
    }
    pub fn setPrefix(self, value: QString) {
        let value = cpp ! (unsafe [self as "QDoubleSpinBox*" , value as "QString"] -> QString as "QString" { self -> setPrefix (value) ; return value ; });
    }
    pub fn singleStep(self) -> double {
        cpp ! (unsafe [self as "QDoubleSpinBox*"] -> double as "double" { return self -> singleStep () ; })
    }
    pub fn setSingleStep(self, value: double) {
        let value = cpp ! (unsafe [self as "QDoubleSpinBox*" , value as "double"] -> double as "double" { self -> setSingleStep (value) ; return value ; });
    }
    pub fn suffix(self) -> QString {
        cpp ! (unsafe [self as "QDoubleSpinBox*"] -> QString as "QString" { return QString (self -> suffix ()) ; })
    }
    pub fn setSuffix(self, value: QString) {
        let value = cpp ! (unsafe [self as "QDoubleSpinBox*" , value as "QString"] -> QString as "QString" { self -> setSuffix (value) ; return value ; });
    }
    pub fn value(self) -> double {
        cpp ! (unsafe [self as "QDoubleSpinBox*"] -> double as "double" { return self -> value () ; })
    }
    pub fn setValue(self, value: double) {
        let value = cpp ! (unsafe [self as "QDoubleSpinBox*" , value as "double"] -> double as "double" { self -> setValue (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QSizeGripPtr as "QPointer<QSizeGrip>");
cpp_class ! (pub unsafe struct QSizeGrip as "QSizeGrip*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QSizeGripPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSizeGrip>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQSizeGrip: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QSizeGrip>>;
}
cpp_class ! (pub unsafe struct QLineEditPtr as "QPointer<QLineEdit>");
cpp_class ! (pub unsafe struct QLineEdit as "QLineEdit*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QLineEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QLineEdit>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQLineEdit: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QLineEdit>>;
}
impl QLineEdit {
    pub fn new() -> Option<QLineEditPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QLineEditPtr) as "std::tuple<bool, QPointer<QLineEdit>>" { QPointer < QLineEdit > btn = new QLineEdit () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QLineEdit> {
    pub fn isClearButtonEnabled(self) -> bool {
        cpp ! (unsafe [self as "QLineEdit*"] -> bool as "bool" { return self -> isClearButtonEnabled () ; })
    }
    pub fn setClearButtonEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QLineEdit*" , value as "bool"] -> bool as "bool" { self -> setClearButtonEnabled (value) ; return value ; });
    }
    pub fn cursorPosition(self) -> int {
        cpp ! (unsafe [self as "QLineEdit*"] -> int as "int" { return self -> cursorPosition () ; })
    }
    pub fn setCursorPosition(self, value: int) {
        let value = cpp ! (unsafe [self as "QLineEdit*" , value as "int"] -> int as "int" { self -> setCursorPosition (value) ; return value ; });
    }
    pub fn dragEnabled(self) -> bool {
        cpp ! (unsafe [self as "QLineEdit*"] -> bool as "bool" { return self -> dragEnabled () ; })
    }
    pub fn setDragEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QLineEdit*" , value as "bool"] -> bool as "bool" { self -> setDragEnabled (value) ; return value ; });
    }
    pub fn hasFrame(self) -> bool {
        cpp ! (unsafe [self as "QLineEdit*"] -> bool as "bool" { return self -> hasFrame () ; })
    }
    pub fn setFrame(self, value: bool) {
        let value = cpp ! (unsafe [self as "QLineEdit*" , value as "bool"] -> bool as "bool" { self -> setFrame (value) ; return value ; });
    }
    pub fn inputMask(self) -> QString {
        cpp ! (unsafe [self as "QLineEdit*"] -> QString as "QString" { return QString (self -> inputMask ()) ; })
    }
    pub fn setInputMask(self, value: QString) {
        let value = cpp ! (unsafe [self as "QLineEdit*" , value as "QString"] -> QString as "QString" { self -> setInputMask (value) ; return value ; });
    }
    pub fn maxLength(self) -> int {
        cpp ! (unsafe [self as "QLineEdit*"] -> int as "int" { return self -> maxLength () ; })
    }
    pub fn setMaxLength(self, value: int) {
        let value = cpp ! (unsafe [self as "QLineEdit*" , value as "int"] -> int as "int" { self -> setMaxLength (value) ; return value ; });
    }
    pub fn isModified(self) -> bool {
        cpp ! (unsafe [self as "QLineEdit*"] -> bool as "bool" { return self -> isModified () ; })
    }
    pub fn setModified(self, value: bool) {
        let value = cpp ! (unsafe [self as "QLineEdit*" , value as "bool"] -> bool as "bool" { self -> setModified (value) ; return value ; });
    }
    pub fn placeholderText(self) -> QString {
        cpp ! (unsafe [self as "QLineEdit*"] -> QString as "QString" { return QString (self -> placeholderText ()) ; })
    }
    pub fn setPlaceholderText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QLineEdit*" , value as "QString"] -> QString as "QString" { self -> setPlaceholderText (value) ; return value ; });
    }
    pub fn isReadOnly(self) -> bool {
        cpp ! (unsafe [self as "QLineEdit*"] -> bool as "bool" { return self -> isReadOnly () ; })
    }
    pub fn setReadOnly(self, value: bool) {
        let value = cpp ! (unsafe [self as "QLineEdit*" , value as "bool"] -> bool as "bool" { self -> setReadOnly (value) ; return value ; });
    }
    pub fn text(self) -> QString {
        cpp ! (unsafe [self as "QLineEdit*"] -> QString as "QString" { return QString (self -> text ()) ; })
    }
    pub fn setText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QLineEdit*" , value as "QString"] -> QString as "QString" { self -> setText (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QFontComboBoxPtr as "QPointer<QFontComboBox>");
cpp_class ! (pub unsafe struct QFontComboBox as "QFontComboBox*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QFontComboBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFontComboBox>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQFontComboBox: IQComboBox {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFontComboBox>>;
}
impl QFontComboBox {
    pub fn new() -> Option<QFontComboBoxPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QFontComboBoxPtr) as "std::tuple<bool, QPointer<QFontComboBox>>" { QPointer < QFontComboBox > btn = new QFontComboBox () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
cpp_class ! (pub unsafe struct QTextBrowserPtr as "QPointer<QTextBrowser>");
cpp_class ! (pub unsafe struct QTextBrowser as "QTextBrowser*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QTextBrowserPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextBrowser>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQTextBrowser: IQTextEdit {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QTextBrowser>>;
}
impl QTextBrowser {
    pub fn new() -> Option<QTextBrowserPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QTextBrowserPtr) as "std::tuple<bool, QPointer<QTextBrowser>>" { QPointer < QTextBrowser > btn = new QTextBrowser () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QTextBrowser> {
    pub fn openExternalLinks(self) -> bool {
        cpp ! (unsafe [self as "QTextBrowser*"] -> bool as "bool" { return self -> openExternalLinks () ; })
    }
    pub fn setOpenExternalLinks(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTextBrowser*" , value as "bool"] -> bool as "bool" { self -> setOpenExternalLinks (value) ; return value ; });
    }
    pub fn openLinks(self) -> bool {
        cpp ! (unsafe [self as "QTextBrowser*"] -> bool as "bool" { return self -> openLinks () ; })
    }
    pub fn setOpenLinks(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTextBrowser*" , value as "bool"] -> bool as "bool" { self -> setOpenLinks (value) ; return value ; });
    }
    pub fn source(self) -> QUrl {
        cpp ! (unsafe [self as "QTextBrowser*"] -> QUrl as "QUrl" { return QUrl (self -> source ()) ; })
    }
    pub fn setSource(self, value: QUrl) {
        let value = cpp ! (unsafe [self as "QTextBrowser*" , value as "QUrl"] -> QUrl as "QUrl" { self -> setSource (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QScrollBarPtr as "QPointer<QScrollBar>");
cpp_class ! (pub unsafe struct QScrollBar as "QScrollBar*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QScrollBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QScrollBar>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQScrollBar: IQAbstractSlider {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QScrollBar>>;
}
impl QScrollBar {
    pub fn new() -> Option<QScrollBarPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QScrollBarPtr) as "std::tuple<bool, QPointer<QScrollBar>>" { QPointer < QScrollBar > btn = new QScrollBar () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
cpp_class ! (pub unsafe struct QInputDialogPtr as "QPointer<QInputDialog>");
cpp_class ! (pub unsafe struct QInputDialog as "QInputDialog*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QInputDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QInputDialog>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQInputDialog: IQDialog {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QInputDialog>>;
}
impl QInputDialog {
    pub fn new() -> Option<QInputDialogPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QInputDialogPtr) as "std::tuple<bool, QPointer<QInputDialog>>" { QPointer < QInputDialog > btn = new QInputDialog () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QInputDialog> {
    pub fn cancelButtonText(self) -> QString {
        cpp ! (unsafe [self as "QInputDialog*"] -> QString as "QString" { return QString (self -> cancelButtonText ()) ; })
    }
    pub fn setCancelButtonText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "QString"] -> QString as "QString" { self -> setCancelButtonText (value) ; return value ; });
    }
    pub fn isComboBoxEditable(self) -> bool {
        cpp ! (unsafe [self as "QInputDialog*"] -> bool as "bool" { return self -> isComboBoxEditable () ; })
    }
    pub fn setComboBoxEditable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "bool"] -> bool as "bool" { self -> setComboBoxEditable (value) ; return value ; });
    }
    pub fn doubleDecimals(self) -> int {
        cpp ! (unsafe [self as "QInputDialog*"] -> int as "int" { return self -> doubleDecimals () ; })
    }
    pub fn setDoubleDecimals(self, value: int) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "int"] -> int as "int" { self -> setDoubleDecimals (value) ; return value ; });
    }
    pub fn doubleMaximum(self) -> double {
        cpp ! (unsafe [self as "QInputDialog*"] -> double as "double" { return self -> doubleMaximum () ; })
    }
    pub fn setDoubleMaximum(self, value: double) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "double"] -> double as "double" { self -> setDoubleMaximum (value) ; return value ; });
    }
    pub fn doubleMinimum(self) -> double {
        cpp ! (unsafe [self as "QInputDialog*"] -> double as "double" { return self -> doubleMinimum () ; })
    }
    pub fn setDoubleMinimum(self, value: double) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "double"] -> double as "double" { self -> setDoubleMinimum (value) ; return value ; });
    }
    pub fn doubleStep(self) -> double {
        cpp ! (unsafe [self as "QInputDialog*"] -> double as "double" { return self -> doubleStep () ; })
    }
    pub fn setDoubleStep(self, value: double) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "double"] -> double as "double" { self -> setDoubleStep (value) ; return value ; });
    }
    pub fn doubleValue(self) -> double {
        cpp ! (unsafe [self as "QInputDialog*"] -> double as "double" { return self -> doubleValue () ; })
    }
    pub fn setDoubleValue(self, value: double) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "double"] -> double as "double" { self -> setDoubleValue (value) ; return value ; });
    }
    pub fn intMaximum(self) -> int {
        cpp ! (unsafe [self as "QInputDialog*"] -> int as "int" { return self -> intMaximum () ; })
    }
    pub fn setIntMaximum(self, value: int) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "int"] -> int as "int" { self -> setIntMaximum (value) ; return value ; });
    }
    pub fn intMinimum(self) -> int {
        cpp ! (unsafe [self as "QInputDialog*"] -> int as "int" { return self -> intMinimum () ; })
    }
    pub fn setIntMinimum(self, value: int) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "int"] -> int as "int" { self -> setIntMinimum (value) ; return value ; });
    }
    pub fn intStep(self) -> int {
        cpp ! (unsafe [self as "QInputDialog*"] -> int as "int" { return self -> intStep () ; })
    }
    pub fn setIntStep(self, value: int) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "int"] -> int as "int" { self -> setIntStep (value) ; return value ; });
    }
    pub fn intValue(self) -> int {
        cpp ! (unsafe [self as "QInputDialog*"] -> int as "int" { return self -> intValue () ; })
    }
    pub fn setIntValue(self, value: int) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "int"] -> int as "int" { self -> setIntValue (value) ; return value ; });
    }
    pub fn labelText(self) -> QString {
        cpp ! (unsafe [self as "QInputDialog*"] -> QString as "QString" { return QString (self -> labelText ()) ; })
    }
    pub fn setLabelText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "QString"] -> QString as "QString" { self -> setLabelText (value) ; return value ; });
    }
    pub fn okButtonText(self) -> QString {
        cpp ! (unsafe [self as "QInputDialog*"] -> QString as "QString" { return QString (self -> okButtonText ()) ; })
    }
    pub fn setOkButtonText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "QString"] -> QString as "QString" { self -> setOkButtonText (value) ; return value ; });
    }
    pub fn textValue(self) -> QString {
        cpp ! (unsafe [self as "QInputDialog*"] -> QString as "QString" { return QString (self -> textValue ()) ; })
    }
    pub fn setTextValue(self, value: QString) {
        let value = cpp ! (unsafe [self as "QInputDialog*" , value as "QString"] -> QString as "QString" { self -> setTextValue (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QAbstractScrollAreaPtr as "QPointer<QAbstractScrollArea>");
cpp_class ! (pub unsafe struct QAbstractScrollArea as "QAbstractScrollArea*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QAbstractScrollAreaPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractScrollArea>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQAbstractScrollArea: IQFrame {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>>;
}
cpp_class ! (pub unsafe struct QTableViewPtr as "QPointer<QTableView>");
cpp_class ! (pub unsafe struct QTableView as "QTableView*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QTableViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableView>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQTableView: IQAbstractItemView {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QTableView>>;
}
impl QTableView {
    pub fn new() -> Option<QTableViewPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QTableViewPtr) as "std::tuple<bool, QPointer<QTableView>>" { QPointer < QTableView > btn = new QTableView () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QTableView> {
    pub fn isCornerButtonEnabled(self) -> bool {
        cpp ! (unsafe [self as "QTableView*"] -> bool as "bool" { return self -> isCornerButtonEnabled () ; })
    }
    pub fn setCornerButtonEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTableView*" , value as "bool"] -> bool as "bool" { self -> setCornerButtonEnabled (value) ; return value ; });
    }
    pub fn showGrid(self) -> bool {
        cpp ! (unsafe [self as "QTableView*"] -> bool as "bool" { return self -> showGrid () ; })
    }
    pub fn setShowGrid(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTableView*" , value as "bool"] -> bool as "bool" { self -> setShowGrid (value) ; return value ; });
    }
    pub fn isSortingEnabled(self) -> bool {
        cpp ! (unsafe [self as "QTableView*"] -> bool as "bool" { return self -> isSortingEnabled () ; })
    }
    pub fn setSortingEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTableView*" , value as "bool"] -> bool as "bool" { self -> setSortingEnabled (value) ; return value ; });
    }
    pub fn wordWrap(self) -> bool {
        cpp ! (unsafe [self as "QTableView*"] -> bool as "bool" { return self -> wordWrap () ; })
    }
    pub fn setWordWrap(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTableView*" , value as "bool"] -> bool as "bool" { self -> setWordWrap (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QProgressDialogPtr as "QPointer<QProgressDialog>");
cpp_class ! (pub unsafe struct QProgressDialog as "QProgressDialog*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QProgressDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QProgressDialog>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQProgressDialog: IQDialog {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QProgressDialog>>;
}
impl QProgressDialog {
    pub fn new() -> Option<QProgressDialogPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QProgressDialogPtr) as "std::tuple<bool, QPointer<QProgressDialog>>" { QPointer < QProgressDialog > btn = new QProgressDialog () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QProgressDialog> {
    pub fn autoClose(self) -> bool {
        cpp ! (unsafe [self as "QProgressDialog*"] -> bool as "bool" { return self -> autoClose () ; })
    }
    pub fn setAutoClose(self, value: bool) {
        let value = cpp ! (unsafe [self as "QProgressDialog*" , value as "bool"] -> bool as "bool" { self -> setAutoClose (value) ; return value ; });
    }
    pub fn autoReset(self) -> bool {
        cpp ! (unsafe [self as "QProgressDialog*"] -> bool as "bool" { return self -> autoReset () ; })
    }
    pub fn setAutoReset(self, value: bool) {
        let value = cpp ! (unsafe [self as "QProgressDialog*" , value as "bool"] -> bool as "bool" { self -> setAutoReset (value) ; return value ; });
    }
    pub fn labelText(self) -> QString {
        cpp ! (unsafe [self as "QProgressDialog*"] -> QString as "QString" { return QString (self -> labelText ()) ; })
    }
    pub fn setLabelText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QProgressDialog*" , value as "QString"] -> QString as "QString" { self -> setLabelText (value) ; return value ; });
    }
    pub fn maximum(self) -> int {
        cpp ! (unsafe [self as "QProgressDialog*"] -> int as "int" { return self -> maximum () ; })
    }
    pub fn setMaximum(self, value: int) {
        let value = cpp ! (unsafe [self as "QProgressDialog*" , value as "int"] -> int as "int" { self -> setMaximum (value) ; return value ; });
    }
    pub fn minimum(self) -> int {
        cpp ! (unsafe [self as "QProgressDialog*"] -> int as "int" { return self -> minimum () ; })
    }
    pub fn setMinimum(self, value: int) {
        let value = cpp ! (unsafe [self as "QProgressDialog*" , value as "int"] -> int as "int" { self -> setMinimum (value) ; return value ; });
    }
    pub fn minimumDuration(self) -> int {
        cpp ! (unsafe [self as "QProgressDialog*"] -> int as "int" { return self -> minimumDuration () ; })
    }
    pub fn setMinimumDuration(self, value: int) {
        let value = cpp ! (unsafe [self as "QProgressDialog*" , value as "int"] -> int as "int" { self -> setMinimumDuration (value) ; return value ; });
    }
    pub fn value(self) -> int {
        cpp ! (unsafe [self as "QProgressDialog*"] -> int as "int" { return self -> value () ; })
    }
    pub fn setValue(self, value: int) {
        let value = cpp ! (unsafe [self as "QProgressDialog*" , value as "int"] -> int as "int" { self -> setValue (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QSliderPtr as "QPointer<QSlider>");
cpp_class ! (pub unsafe struct QSlider as "QSlider*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QSliderPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSlider>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQSlider: IQAbstractSlider {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QSlider>>;
}
impl QSlider {
    pub fn new() -> Option<QSliderPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QSliderPtr) as "std::tuple<bool, QPointer<QSlider>>" { QPointer < QSlider > btn = new QSlider () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QSlider> {
    pub fn tickInterval(self) -> int {
        cpp ! (unsafe [self as "QSlider*"] -> int as "int" { return self -> tickInterval () ; })
    }
    pub fn setTickInterval(self, value: int) {
        let value = cpp ! (unsafe [self as "QSlider*" , value as "int"] -> int as "int" { self -> setTickInterval (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QDialogPtr as "QPointer<QDialog>");
cpp_class ! (pub unsafe struct QDialog as "QDialog*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDialog>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQDialog: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDialog>>;
}
impl QDialog {
    pub fn new() -> Option<QDialogPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QDialogPtr) as "std::tuple<bool, QPointer<QDialog>>" { QPointer < QDialog > btn = new QDialog () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QDialog> {
    pub fn isModal(self) -> bool {
        cpp ! (unsafe [self as "QDialog*"] -> bool as "bool" { return self -> isModal () ; })
    }
    pub fn setModal(self, value: bool) {
        let value = cpp ! (unsafe [self as "QDialog*" , value as "bool"] -> bool as "bool" { self -> setModal (value) ; return value ; });
    }
    pub fn isSizeGripEnabled(self) -> bool {
        cpp ! (unsafe [self as "QDialog*"] -> bool as "bool" { return self -> isSizeGripEnabled () ; })
    }
    pub fn setSizeGripEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QDialog*" , value as "bool"] -> bool as "bool" { self -> setSizeGripEnabled (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QWizardPtr as "QPointer<QWizard>");
cpp_class ! (pub unsafe struct QWizard as "QWizard*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QWizardPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QWizard>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQWizard: IQDialog {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWizard>>;
}
impl QWizard {
    pub fn new() -> Option<QWizardPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QWizardPtr) as "std::tuple<bool, QPointer<QWizard>>" { QPointer < QWizard > btn = new QWizard () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QWizard> {
    pub fn startId(self) -> int {
        cpp ! (unsafe [self as "QWizard*"] -> int as "int" { return self -> startId () ; })
    }
    pub fn setStartId(self, value: int) {
        let value = cpp ! (unsafe [self as "QWizard*" , value as "int"] -> int as "int" { self -> setStartId (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QCheckBoxPtr as "QPointer<QCheckBox>");
cpp_class ! (pub unsafe struct QCheckBox as "QCheckBox*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QCheckBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QCheckBox>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQCheckBox: IQAbstractButton {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QCheckBox>>;
}
impl QCheckBox {
    pub fn new() -> Option<QCheckBoxPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QCheckBoxPtr) as "std::tuple<bool, QPointer<QCheckBox>>" { QPointer < QCheckBox > btn = new QCheckBox () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QCheckBox> {
    pub fn isTristate(self) -> bool {
        cpp ! (unsafe [self as "QCheckBox*"] -> bool as "bool" { return self -> isTristate () ; })
    }
    pub fn setTristate(self, value: bool) {
        let value = cpp ! (unsafe [self as "QCheckBox*" , value as "bool"] -> bool as "bool" { self -> setTristate (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QSplashScreenPtr as "QPointer<QSplashScreen>");
cpp_class ! (pub unsafe struct QSplashScreen as "QSplashScreen*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QSplashScreenPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSplashScreen>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQSplashScreen: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QSplashScreen>>;
}
impl QSplashScreen {
    pub fn new() -> Option<QSplashScreenPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QSplashScreenPtr) as "std::tuple<bool, QPointer<QSplashScreen>>" { QPointer < QSplashScreen > btn = new QSplashScreen () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
cpp_class ! (pub unsafe struct QErrorMessagePtr as "QPointer<QErrorMessage>");
cpp_class ! (pub unsafe struct QErrorMessage as "QErrorMessage*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QErrorMessagePtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QErrorMessage>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQErrorMessage: IQDialog {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QErrorMessage>>;
}
impl QErrorMessage {
    pub fn new() -> Option<QErrorMessagePtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QErrorMessagePtr) as "std::tuple<bool, QPointer<QErrorMessage>>" { QPointer < QErrorMessage > btn = new QErrorMessage () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
cpp_class ! (pub unsafe struct QSplitterHandlePtr as "QPointer<QSplitterHandle>");
cpp_class ! (pub unsafe struct QSplitterHandle as "QSplitterHandle*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QSplitterHandlePtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSplitterHandle>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQSplitterHandle: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QSplitterHandle>>;
}
cpp_class ! (pub unsafe struct QTabWidgetPtr as "QPointer<QTabWidget>");
cpp_class ! (pub unsafe struct QTabWidget as "QTabWidget*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QTabWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTabWidget>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQTabWidget: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QTabWidget>>;
}
impl QTabWidget {
    pub fn new() -> Option<QTabWidgetPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QTabWidgetPtr) as "std::tuple<bool, QPointer<QTabWidget>>" { QPointer < QTabWidget > btn = new QTabWidget () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QTabWidget> {
    pub fn currentIndex(self) -> int {
        cpp ! (unsafe [self as "QTabWidget*"] -> int as "int" { return self -> currentIndex () ; })
    }
    pub fn setCurrentIndex(self, value: int) {
        let value = cpp ! (unsafe [self as "QTabWidget*" , value as "int"] -> int as "int" { self -> setCurrentIndex (value) ; return value ; });
    }
    pub fn documentMode(self) -> bool {
        cpp ! (unsafe [self as "QTabWidget*"] -> bool as "bool" { return self -> documentMode () ; })
    }
    pub fn setDocumentMode(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabWidget*" , value as "bool"] -> bool as "bool" { self -> setDocumentMode (value) ; return value ; });
    }
    pub fn iconSize(self) -> QSize {
        cpp ! (unsafe [self as "QTabWidget*"] -> QSize as "QSize" { return QSize (self -> iconSize ()) ; })
    }
    pub fn setIconSize(self, value: QSize) {
        let value = cpp ! (unsafe [self as "QTabWidget*" , value as "QSize"] -> QSize as "QSize" { self -> setIconSize (value) ; return value ; });
    }
    pub fn isMovable(self) -> bool {
        cpp ! (unsafe [self as "QTabWidget*"] -> bool as "bool" { return self -> isMovable () ; })
    }
    pub fn setMovable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabWidget*" , value as "bool"] -> bool as "bool" { self -> setMovable (value) ; return value ; });
    }
    pub fn tabBarAutoHide(self) -> bool {
        cpp ! (unsafe [self as "QTabWidget*"] -> bool as "bool" { return self -> tabBarAutoHide () ; })
    }
    pub fn setTabBarAutoHide(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabWidget*" , value as "bool"] -> bool as "bool" { self -> setTabBarAutoHide (value) ; return value ; });
    }
    pub fn tabsClosable(self) -> bool {
        cpp ! (unsafe [self as "QTabWidget*"] -> bool as "bool" { return self -> tabsClosable () ; })
    }
    pub fn setTabsClosable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabWidget*" , value as "bool"] -> bool as "bool" { self -> setTabsClosable (value) ; return value ; });
    }
    pub fn usesScrollButtons(self) -> bool {
        cpp ! (unsafe [self as "QTabWidget*"] -> bool as "bool" { return self -> usesScrollButtons () ; })
    }
    pub fn setUsesScrollButtons(self, value: bool) {
        let value = cpp ! (unsafe [self as "QTabWidget*" , value as "bool"] -> bool as "bool" { self -> setUsesScrollButtons (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QAbstractButtonPtr as "QPointer<QAbstractButton>");
cpp_class ! (pub unsafe struct QAbstractButton as "QAbstractButton*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QAbstractButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractButton>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQAbstractButton: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractButton>>;
}
impl<'a> QRef<'a, QAbstractButton> {
    pub fn autoExclusive(self) -> bool {
        cpp ! (unsafe [self as "QAbstractButton*"] -> bool as "bool" { return self -> autoExclusive () ; })
    }
    pub fn setAutoExclusive(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractButton*" , value as "bool"] -> bool as "bool" { self -> setAutoExclusive (value) ; return value ; });
    }
    pub fn autoRepeat(self) -> bool {
        cpp ! (unsafe [self as "QAbstractButton*"] -> bool as "bool" { return self -> autoRepeat () ; })
    }
    pub fn setAutoRepeat(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractButton*" , value as "bool"] -> bool as "bool" { self -> setAutoRepeat (value) ; return value ; });
    }
    pub fn autoRepeatDelay(self) -> int {
        cpp ! (unsafe [self as "QAbstractButton*"] -> int as "int" { return self -> autoRepeatDelay () ; })
    }
    pub fn setAutoRepeatDelay(self, value: int) {
        let value = cpp ! (unsafe [self as "QAbstractButton*" , value as "int"] -> int as "int" { self -> setAutoRepeatDelay (value) ; return value ; });
    }
    pub fn autoRepeatInterval(self) -> int {
        cpp ! (unsafe [self as "QAbstractButton*"] -> int as "int" { return self -> autoRepeatInterval () ; })
    }
    pub fn setAutoRepeatInterval(self, value: int) {
        let value = cpp ! (unsafe [self as "QAbstractButton*" , value as "int"] -> int as "int" { self -> setAutoRepeatInterval (value) ; return value ; });
    }
    pub fn isCheckable(self) -> bool {
        cpp ! (unsafe [self as "QAbstractButton*"] -> bool as "bool" { return self -> isCheckable () ; })
    }
    pub fn setCheckable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractButton*" , value as "bool"] -> bool as "bool" { self -> setCheckable (value) ; return value ; });
    }
    pub fn isChecked(self) -> bool {
        cpp ! (unsafe [self as "QAbstractButton*"] -> bool as "bool" { return self -> isChecked () ; })
    }
    pub fn setChecked(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractButton*" , value as "bool"] -> bool as "bool" { self -> setChecked (value) ; return value ; });
    }
    pub fn isDown(self) -> bool {
        cpp ! (unsafe [self as "QAbstractButton*"] -> bool as "bool" { return self -> isDown () ; })
    }
    pub fn setDown(self, value: bool) {
        let value = cpp ! (unsafe [self as "QAbstractButton*" , value as "bool"] -> bool as "bool" { self -> setDown (value) ; return value ; });
    }
    pub fn iconSize(self) -> QSize {
        cpp ! (unsafe [self as "QAbstractButton*"] -> QSize as "QSize" { return QSize (self -> iconSize ()) ; })
    }
    pub fn setIconSize(self, value: QSize) {
        let value = cpp ! (unsafe [self as "QAbstractButton*" , value as "QSize"] -> QSize as "QSize" { self -> setIconSize (value) ; return value ; });
    }
    pub fn text(self) -> QString {
        cpp ! (unsafe [self as "QAbstractButton*"] -> QString as "QString" { return QString (self -> text ()) ; })
    }
    pub fn setText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QAbstractButton*" , value as "QString"] -> QString as "QString" { self -> setText (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QGraphicsViewPtr as "QPointer<QGraphicsView>");
cpp_class ! (pub unsafe struct QGraphicsView as "QGraphicsView*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QGraphicsViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QGraphicsView>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQGraphicsView: IQAbstractScrollArea {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QGraphicsView>>;
}
impl QGraphicsView {
    pub fn new() -> Option<QGraphicsViewPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QGraphicsViewPtr) as "std::tuple<bool, QPointer<QGraphicsView>>" { QPointer < QGraphicsView > btn = new QGraphicsView () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QGraphicsView> {
    pub fn isInteractive(self) -> bool {
        cpp ! (unsafe [self as "QGraphicsView*"] -> bool as "bool" { return self -> isInteractive () ; })
    }
    pub fn setInteractive(self, value: bool) {
        let value = cpp ! (unsafe [self as "QGraphicsView*" , value as "bool"] -> bool as "bool" { self -> setInteractive (value) ; return value ; });
    }
    pub fn sceneRect(self) -> QRectF {
        cpp ! (unsafe [self as "QGraphicsView*"] -> QRectF as "QRectF" { return QRectF (self -> sceneRect ()) ; })
    }
    pub fn setSceneRect(self, value: QRectF) {
        let value = cpp ! (unsafe [self as "QGraphicsView*" , value as "QRectF"] -> QRectF as "QRectF" { self -> setSceneRect (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QFontDialogPtr as "QPointer<QFontDialog>");
cpp_class ! (pub unsafe struct QFontDialog as "QFontDialog*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QFontDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFontDialog>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQFontDialog: IQDialog {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFontDialog>>;
}
impl QFontDialog {
    pub fn new() -> Option<QFontDialogPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QFontDialogPtr) as "std::tuple<bool, QPointer<QFontDialog>>" { QPointer < QFontDialog > btn = new QFontDialog () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
cpp_class ! (pub unsafe struct QDialPtr as "QPointer<QDial>");
cpp_class ! (pub unsafe struct QDial as "QDial*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QDialPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDial>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQDial: IQAbstractSlider {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDial>>;
}
impl QDial {
    pub fn new() -> Option<QDialPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QDialPtr) as "std::tuple<bool, QPointer<QDial>>" { QPointer < QDial > btn = new QDial () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QDial> {
    pub fn notchTarget(self) -> qreal {
        cpp ! (unsafe [self as "QDial*"] -> qreal as "qreal" { return self -> notchTarget () ; })
    }
    pub fn setNotchTarget(self, value: qreal) {
        let value = cpp ! (unsafe [self as "QDial*" , value as "qreal"] -> qreal as "qreal" { self -> setNotchTarget (value) ; return value ; });
    }
    pub fn notchesVisible(self) -> bool {
        cpp ! (unsafe [self as "QDial*"] -> bool as "bool" { return self -> notchesVisible () ; })
    }
    pub fn setNotchesVisible(self, value: bool) {
        let value = cpp ! (unsafe [self as "QDial*" , value as "bool"] -> bool as "bool" { self -> setNotchesVisible (value) ; return value ; });
    }
    pub fn wrapping(self) -> bool {
        cpp ! (unsafe [self as "QDial*"] -> bool as "bool" { return self -> wrapping () ; })
    }
    pub fn setWrapping(self, value: bool) {
        let value = cpp ! (unsafe [self as "QDial*" , value as "bool"] -> bool as "bool" { self -> setWrapping (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QColumnViewPtr as "QPointer<QColumnView>");
cpp_class ! (pub unsafe struct QColumnView as "QColumnView*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QColumnViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QColumnView>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQColumnView: IQAbstractItemView {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QColumnView>>;
}
impl QColumnView {
    pub fn new() -> Option<QColumnViewPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QColumnViewPtr) as "std::tuple<bool, QPointer<QColumnView>>" { QPointer < QColumnView > btn = new QColumnView () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QColumnView> {
    pub fn resizeGripsVisible(self) -> bool {
        cpp ! (unsafe [self as "QColumnView*"] -> bool as "bool" { return self -> resizeGripsVisible () ; })
    }
    pub fn setResizeGripsVisible(self, value: bool) {
        let value = cpp ! (unsafe [self as "QColumnView*" , value as "bool"] -> bool as "bool" { self -> setResizeGripsVisible (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QScrollAreaPtr as "QPointer<QScrollArea>");
cpp_class ! (pub unsafe struct QScrollArea as "QScrollArea*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QScrollAreaPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QScrollArea>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQScrollArea: IQAbstractScrollArea {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QScrollArea>>;
}
impl QScrollArea {
    pub fn new() -> Option<QScrollAreaPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QScrollAreaPtr) as "std::tuple<bool, QPointer<QScrollArea>>" { QPointer < QScrollArea > btn = new QScrollArea () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QScrollArea> {
    pub fn widgetResizable(self) -> bool {
        cpp ! (unsafe [self as "QScrollArea*"] -> bool as "bool" { return self -> widgetResizable () ; })
    }
    pub fn setWidgetResizable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QScrollArea*" , value as "bool"] -> bool as "bool" { self -> setWidgetResizable (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QFocusFramePtr as "QPointer<QFocusFrame>");
cpp_class ! (pub unsafe struct QFocusFrame as "QFocusFrame*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QFocusFramePtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFocusFrame>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQFocusFrame: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFocusFrame>>;
}
impl QFocusFrame {
    pub fn new() -> Option<QFocusFramePtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QFocusFramePtr) as "std::tuple<bool, QPointer<QFocusFrame>>" { QPointer < QFocusFrame > btn = new QFocusFrame () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
cpp_class ! (pub unsafe struct QCommandLinkButtonPtr as "QPointer<QCommandLinkButton>");
cpp_class ! (pub unsafe struct QCommandLinkButton as "QCommandLinkButton*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QCommandLinkButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QCommandLinkButton>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQCommandLinkButton: IQPushButton {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QCommandLinkButton>>;
}
impl QCommandLinkButton {
    pub fn new() -> Option<QCommandLinkButtonPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QCommandLinkButtonPtr) as "std::tuple<bool, QPointer<QCommandLinkButton>>" { QPointer < QCommandLinkButton > btn = new QCommandLinkButton () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QCommandLinkButton> {
    pub fn description(self) -> QString {
        cpp ! (unsafe [self as "QCommandLinkButton*"] -> QString as "QString" { return QString (self -> description ()) ; })
    }
    pub fn setDescription(self, value: QString) {
        let value = cpp ! (unsafe [self as "QCommandLinkButton*" , value as "QString"] -> QString as "QString" { self -> setDescription (value) ; return value ; });
    }
    pub fn isFlat(self) -> bool {
        cpp ! (unsafe [self as "QCommandLinkButton*"] -> bool as "bool" { return self -> isFlat () ; })
    }
    pub fn setFlat(self, value: bool) {
        let value = cpp ! (unsafe [self as "QCommandLinkButton*" , value as "bool"] -> bool as "bool" { self -> setFlat (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QComboBoxPtr as "QPointer<QComboBox>");
cpp_class ! (pub unsafe struct QComboBox as "QComboBox*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QComboBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QComboBox>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQComboBox: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QComboBox>>;
}
impl QComboBox {
    pub fn new() -> Option<QComboBoxPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QComboBoxPtr) as "std::tuple<bool, QPointer<QComboBox>>" { QPointer < QComboBox > btn = new QComboBox () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QComboBox> {
    pub fn currentIndex(self) -> int {
        cpp ! (unsafe [self as "QComboBox*"] -> int as "int" { return self -> currentIndex () ; })
    }
    pub fn setCurrentIndex(self, value: int) {
        let value = cpp ! (unsafe [self as "QComboBox*" , value as "int"] -> int as "int" { self -> setCurrentIndex (value) ; return value ; });
    }
    pub fn currentText(self) -> QString {
        cpp ! (unsafe [self as "QComboBox*"] -> QString as "QString" { return QString (self -> currentText ()) ; })
    }
    pub fn setCurrentText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QComboBox*" , value as "QString"] -> QString as "QString" { self -> setCurrentText (value) ; return value ; });
    }
    pub fn duplicatesEnabled(self) -> bool {
        cpp ! (unsafe [self as "QComboBox*"] -> bool as "bool" { return self -> duplicatesEnabled () ; })
    }
    pub fn setDuplicatesEnabled(self, value: bool) {
        let value = cpp ! (unsafe [self as "QComboBox*" , value as "bool"] -> bool as "bool" { self -> setDuplicatesEnabled (value) ; return value ; });
    }
    pub fn isEditable(self) -> bool {
        cpp ! (unsafe [self as "QComboBox*"] -> bool as "bool" { return self -> isEditable () ; })
    }
    pub fn setEditable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QComboBox*" , value as "bool"] -> bool as "bool" { self -> setEditable (value) ; return value ; });
    }
    pub fn hasFrame(self) -> bool {
        cpp ! (unsafe [self as "QComboBox*"] -> bool as "bool" { return self -> hasFrame () ; })
    }
    pub fn setFrame(self, value: bool) {
        let value = cpp ! (unsafe [self as "QComboBox*" , value as "bool"] -> bool as "bool" { self -> setFrame (value) ; return value ; });
    }
    pub fn iconSize(self) -> QSize {
        cpp ! (unsafe [self as "QComboBox*"] -> QSize as "QSize" { return QSize (self -> iconSize ()) ; })
    }
    pub fn setIconSize(self, value: QSize) {
        let value = cpp ! (unsafe [self as "QComboBox*" , value as "QSize"] -> QSize as "QSize" { self -> setIconSize (value) ; return value ; });
    }
    pub fn maxCount(self) -> int {
        cpp ! (unsafe [self as "QComboBox*"] -> int as "int" { return self -> maxCount () ; })
    }
    pub fn setMaxCount(self, value: int) {
        let value = cpp ! (unsafe [self as "QComboBox*" , value as "int"] -> int as "int" { self -> setMaxCount (value) ; return value ; });
    }
    pub fn maxVisibleItems(self) -> int {
        cpp ! (unsafe [self as "QComboBox*"] -> int as "int" { return self -> maxVisibleItems () ; })
    }
    pub fn setMaxVisibleItems(self, value: int) {
        let value = cpp ! (unsafe [self as "QComboBox*" , value as "int"] -> int as "int" { self -> setMaxVisibleItems (value) ; return value ; });
    }
    pub fn minimumContentsLength(self) -> int {
        cpp ! (unsafe [self as "QComboBox*"] -> int as "int" { return self -> minimumContentsLength () ; })
    }
    pub fn setMinimumContentsLength(self, value: int) {
        let value = cpp ! (unsafe [self as "QComboBox*" , value as "int"] -> int as "int" { self -> setMinimumContentsLength (value) ; return value ; });
    }
    pub fn modelColumn(self) -> int {
        cpp ! (unsafe [self as "QComboBox*"] -> int as "int" { return self -> modelColumn () ; })
    }
    pub fn setModelColumn(self, value: int) {
        let value = cpp ! (unsafe [self as "QComboBox*" , value as "int"] -> int as "int" { self -> setModelColumn (value) ; return value ; });
    }
    pub fn placeholderText(self) -> QString {
        cpp ! (unsafe [self as "QComboBox*"] -> QString as "QString" { return QString (self -> placeholderText ()) ; })
    }
    pub fn setPlaceholderText(self, value: QString) {
        let value = cpp ! (unsafe [self as "QComboBox*" , value as "QString"] -> QString as "QString" { self -> setPlaceholderText (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QToolBarPtr as "QPointer<QToolBar>");
cpp_class ! (pub unsafe struct QToolBar as "QToolBar*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QToolBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolBar>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQToolBar: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QToolBar>>;
}
impl QToolBar {
    pub fn new() -> Option<QToolBarPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QToolBarPtr) as "std::tuple<bool, QPointer<QToolBar>>" { QPointer < QToolBar > btn = new QToolBar () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QToolBar> {
    pub fn isFloatable(self) -> bool {
        cpp ! (unsafe [self as "QToolBar*"] -> bool as "bool" { return self -> isFloatable () ; })
    }
    pub fn setFloatable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QToolBar*" , value as "bool"] -> bool as "bool" { self -> setFloatable (value) ; return value ; });
    }
    pub fn iconSize(self) -> QSize {
        cpp ! (unsafe [self as "QToolBar*"] -> QSize as "QSize" { return QSize (self -> iconSize ()) ; })
    }
    pub fn setIconSize(self, value: QSize) {
        let value = cpp ! (unsafe [self as "QToolBar*" , value as "QSize"] -> QSize as "QSize" { self -> setIconSize (value) ; return value ; });
    }
    pub fn isMovable(self) -> bool {
        cpp ! (unsafe [self as "QToolBar*"] -> bool as "bool" { return self -> isMovable () ; })
    }
    pub fn setMovable(self, value: bool) {
        let value = cpp ! (unsafe [self as "QToolBar*" , value as "bool"] -> bool as "bool" { self -> setMovable (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QColorDialogPtr as "QPointer<QColorDialog>");
cpp_class ! (pub unsafe struct QColorDialog as "QColorDialog*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QColorDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QColorDialog>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQColorDialog: IQDialog {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QColorDialog>>;
}
impl QColorDialog {
    pub fn new() -> Option<QColorDialogPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QColorDialogPtr) as "std::tuple<bool, QPointer<QColorDialog>>" { QPointer < QColorDialog > btn = new QColorDialog () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QColorDialog> {
    pub fn currentColor(self) -> QColor {
        cpp ! (unsafe [self as "QColorDialog*"] -> QColor as "QColor" { return QColor (self -> currentColor ()) ; })
    }
    pub fn setCurrentColor(self, value: QColor) {
        let value = cpp ! (unsafe [self as "QColorDialog*" , value as "QColor"] -> QColor as "QColor" { self -> setCurrentColor (value) ; return value ; });
    }
}
cpp_class ! (pub unsafe struct QKeySequenceEditPtr as "QPointer<QKeySequenceEdit>");
cpp_class ! (pub unsafe struct QKeySequenceEdit as "QKeySequenceEdit*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QKeySequenceEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QKeySequenceEdit>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQKeySequenceEdit: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QKeySequenceEdit>>;
}
impl QKeySequenceEdit {
    pub fn new() -> Option<QKeySequenceEditPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QKeySequenceEditPtr) as "std::tuple<bool, QPointer<QKeySequenceEdit>>" { QPointer < QKeySequenceEdit > btn = new QKeySequenceEdit () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
cpp_class ! (pub unsafe struct QDialogButtonBoxPtr as "QPointer<QDialogButtonBox>");
cpp_class ! (pub unsafe struct QDialogButtonBox as "QDialogButtonBox*");
#[dyn_cast(
    IQListView,
    IQGraphicsView,
    IQInputDialog,
    IQColumnView,
    IQAbstractButton,
    IQLineEdit,
    IQDateEdit,
    IQPushButton,
    IQDialogButtonBox,
    IQTabWidget,
    IQSizeGrip,
    IQFontDialog,
    IQSplitterHandle,
    IQAbstractScrollArea,
    IQStackedWidget,
    IQFileDialog,
    IQProgressBar,
    IQSplashScreen,
    IQPlainTextEdit,
    IQSplitter,
    IQDateTimeEdit,
    IQFrame,
    IQProgressDialog,
    IQToolBar,
    IQTextBrowser,
    IQSpinBox,
    IQScrollBar,
    IQMenuBar,
    IQCommandLinkButton,
    IQRadioButton,
    IQTableView,
    IQTreeView,
    IQToolBox,
    IQSlider,
    IQOpenGLWidget,
    IQAbstractSpinBox,
    IQWizardPage,
    IQErrorMessage,
    IQScrollArea,
    IQMessageBox,
    IQAbstractItemView,
    IQMdiArea,
    IQTableWidget,
    IQWizard,
    IQTreeWidget,
    IQLabel,
    IQFocusFrame,
    IQDoubleSpinBox,
    IQToolButton,
    IQComboBox,
    IQMdiSubWindow,
    IQMainWindow,
    IQHeaderView,
    IQGroupBox,
    IQAbstractSlider,
    IQDialog,
    IQRubberBand,
    IQCheckBox,
    IQStatusBar,
    IQTimeEdit,
    IQCalendarWidget,
    IQListWidget,
    IQColorDialog,
    IQDockWidget,
    IQUndoView,
    IQLCDNumber,
    IQTextEdit,
    IQDial,
    IQWidget,
    IQFontComboBox,
    IQMenu,
    IQTabBar,
    IQKeySequenceEdit
)]
#[dyn_upcast]
impl IQObject for QDialogButtonBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QObject> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDialogButtonBox>*"] -> (bool , QObject) as "std::tuple<bool, QObject*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QObject *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QObject * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
pub trait IQDialogButtonBox: IQWidget {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDialogButtonBox>>;
}
impl QDialogButtonBox {
    pub fn new() -> Option<QDialogButtonBoxPtr> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([] -> (bool , QDialogButtonBoxPtr) as "std::tuple<bool, QPointer<QDialogButtonBox>>" { QPointer < QDialogButtonBox > btn = new QDialogButtonBox () ; return std :: make_tuple (! btn . isNull () , btn) ; });
            if is_safe {
                Some(ptr)
            } else {
                None
            }
        }
    }
}
impl<'a> QRef<'a, QDialogButtonBox> {
    pub fn centerButtons(self) -> bool {
        cpp ! (unsafe [self as "QDialogButtonBox*"] -> bool as "bool" { return self -> centerButtons () ; })
    }
    pub fn setCenterButtons(self, value: bool) {
        let value = cpp ! (unsafe [self as "QDialogButtonBox*" , value as "bool"] -> bool as "bool" { self -> setCenterButtons (value) ; return value ; });
    }
}
impl IQWidget for QStatusBarPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QStatusBar>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QWizardPagePtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QWizardPage>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractItemView for QListViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractItemView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QListView>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QListViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QListView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QListViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QListView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QListViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QListView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractItemView for QTableViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractItemView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableView>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QTableViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QTableViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QTableViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QSplitterHandlePtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QSplitterHandle>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QDialogPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QDialog>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractButton for QRadioButtonPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractButton>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QRadioButton>*"] -> (bool , QAbstractButton) as "std::tuple<bool, QAbstractButton*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractButton *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractButton * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QRadioButtonPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QRadioButton>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QSizeGripPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QSizeGrip>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractItemView for QTreeViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractItemView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeView>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QTreeViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QTreeViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QTreeViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QLabelPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QLabel>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QLabelPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QLabel>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QTabBarPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTabBar>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQTreeView for QTreeWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QTreeView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeWidget>*"] -> (bool , QTreeView) as "std::tuple<bool, QTreeView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QTreeView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QTreeView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractItemView for QTreeWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractItemView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeWidget>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QTreeWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeWidget>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QTreeWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeWidget>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QTreeWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQPushButton for QCommandLinkButtonPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QPushButton>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QCommandLinkButton>*"] -> (bool , QPushButton) as "std::tuple<bool, QPushButton*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QPushButton *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QPushButton * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractButton for QCommandLinkButtonPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractButton>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QCommandLinkButton>*"] -> (bool , QAbstractButton) as "std::tuple<bool, QAbstractButton*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractButton *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractButton * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QCommandLinkButtonPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QCommandLinkButton>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQTableView for QTableWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QTableView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableWidget>*"] -> (bool , QTableView) as "std::tuple<bool, QTableView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QTableView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QTableView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractItemView for QTableWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractItemView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableWidget>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QTableWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableWidget>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QTableWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableWidget>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QTableWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractItemView for QColumnViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractItemView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QColumnView>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QColumnViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QColumnView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QColumnViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QColumnView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QColumnViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QColumnView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QMdiAreaPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QMdiArea>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QMdiAreaPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QMdiArea>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QMdiAreaPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QMdiArea>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQDialog for QColorDialogPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDialog>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QColorDialog>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QColorDialogPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QColorDialog>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QToolBarPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolBar>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractSpinBox for QSpinBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractSpinBox>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QSpinBox>*"] -> (bool , QAbstractSpinBox) as "std::tuple<bool, QAbstractSpinBox*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSpinBox *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSpinBox * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QSpinBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QSpinBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QFramePtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QFrame>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQDateTimeEdit for QDateEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDateTimeEdit>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QDateEdit>*"] -> (bool , QDateTimeEdit) as "std::tuple<bool, QDateTimeEdit*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDateTimeEdit *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDateTimeEdit * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractSpinBox for QDateEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractSpinBox>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QDateEdit>*"] -> (bool , QAbstractSpinBox) as "std::tuple<bool, QAbstractSpinBox*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSpinBox *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSpinBox * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QDateEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QDateEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQListView for QUndoViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QListView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QUndoView>*"] -> (bool , QListView) as "std::tuple<bool, QListView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QListView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QListView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractItemView for QUndoViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractItemView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QUndoView>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QUndoViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QUndoView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QUndoViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QUndoView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QUndoViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QUndoView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QTextEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextEdit>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QTextEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextEdit>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QTextEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQTextEdit for QTextBrowserPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QTextEdit>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextBrowser>*"] -> (bool , QTextEdit) as "std::tuple<bool, QTextEdit*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QTextEdit *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QTextEdit * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QTextBrowserPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextBrowser>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QTextBrowserPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextBrowser>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QTextBrowserPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextBrowser>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QLineEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QLineEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractButton for QPushButtonPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractButton>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QPushButton>*"] -> (bool , QAbstractButton) as "std::tuple<bool, QAbstractButton*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractButton *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractButton * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QPushButtonPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QPushButton>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QStackedWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QStackedWidget>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QStackedWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QStackedWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractItemView for QHeaderViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractItemView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QHeaderView>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QHeaderViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QHeaderView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QHeaderViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QHeaderView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QHeaderViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QHeaderView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QSplitterPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QSplitter>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QSplitterPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QSplitter>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQDialog for QFontDialogPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDialog>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QFontDialog>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QFontDialogPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QFontDialog>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QScrollAreaPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QScrollArea>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QScrollAreaPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QScrollArea>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QScrollAreaPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QScrollArea>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QPlainTextEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QPlainTextEdit>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QPlainTextEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QPlainTextEdit>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QPlainTextEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QPlainTextEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQComboBox for QFontComboBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QComboBox>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QFontComboBox>*"] -> (bool , QComboBox) as "std::tuple<bool, QComboBox*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QComboBox *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QComboBox * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QFontComboBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QFontComboBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractSpinBox for QDoubleSpinBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractSpinBox>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QDoubleSpinBox>*"] -> (bool , QAbstractSpinBox) as "std::tuple<bool, QAbstractSpinBox*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSpinBox *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSpinBox * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QDoubleSpinBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QDoubleSpinBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQDialog for QMessageBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDialog>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QMessageBox>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QMessageBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QMessageBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QAbstractSliderPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractSlider>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQDateTimeEdit for QTimeEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDateTimeEdit>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTimeEdit>*"] -> (bool , QDateTimeEdit) as "std::tuple<bool, QDateTimeEdit*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDateTimeEdit *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDateTimeEdit * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractSpinBox for QTimeEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractSpinBox>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTimeEdit>*"] -> (bool , QAbstractSpinBox) as "std::tuple<bool, QAbstractSpinBox*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSpinBox *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSpinBox * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QTimeEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTimeEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QMdiSubWindowPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QMdiSubWindow>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QAbstractButtonPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractButton>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QDockWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QDockWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QAbstractScrollAreaPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractScrollArea>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QAbstractScrollAreaPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractScrollArea>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QMenuPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QMenu>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QSplashScreenPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QSplashScreen>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QLCDNumberPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QLCDNumber>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QLCDNumberPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QLCDNumber>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QProgressBarPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QProgressBar>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QOpenGLWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QOpenGLWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractButton for QCheckBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractButton>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QCheckBox>*"] -> (bool , QAbstractButton) as "std::tuple<bool, QAbstractButton*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractButton *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractButton * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QCheckBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QCheckBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQListView for QListWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QListView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QListWidget>*"] -> (bool , QListView) as "std::tuple<bool, QListView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QListView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QListView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractItemView for QListWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractItemView>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QListWidget>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QListWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QListWidget>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QListWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QListWidget>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QListWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QListWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQDialog for QErrorMessagePtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDialog>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QErrorMessage>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QErrorMessagePtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QErrorMessage>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QToolBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolBox>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QToolBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractSlider for QScrollBarPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractSlider>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QScrollBar>*"] -> (bool , QAbstractSlider) as "std::tuple<bool, QAbstractSlider*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSlider *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSlider * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QScrollBarPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QScrollBar>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QMenuBarPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QMenuBar>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQDialog for QProgressDialogPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDialog>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QProgressDialog>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QProgressDialogPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QProgressDialog>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QAbstractItemViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractItemView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QAbstractItemViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractItemView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QAbstractItemViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractItemView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QDialogButtonBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QDialogButtonBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QComboBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QComboBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractSpinBox for QDateTimeEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractSpinBox>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QDateTimeEdit>*"] -> (bool , QAbstractSpinBox) as "std::tuple<bool, QAbstractSpinBox*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSpinBox *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSpinBox * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QDateTimeEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QDateTimeEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QTabWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QTabWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQDialog for QWizardPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDialog>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QWizard>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QWizardPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QWizard>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QRubberBandPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QRubberBand>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractSlider for QDialPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractSlider>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QDial>*"] -> (bool , QAbstractSlider) as "std::tuple<bool, QAbstractSlider*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSlider *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSlider * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QDialPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QDial>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractSlider for QSliderPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractSlider>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QSlider>*"] -> (bool , QAbstractSlider) as "std::tuple<bool, QAbstractSlider*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSlider *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSlider * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QSliderPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QSlider>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQDialog for QFileDialogPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDialog>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QFileDialog>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QFileDialogPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QFileDialog>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QAbstractSpinBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractSpinBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQDialog for QInputDialogPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QDialog>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QInputDialog>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QInputDialogPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QInputDialog>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QKeySequenceEditPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QKeySequenceEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractScrollArea for QGraphicsViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractScrollArea>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QGraphicsView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQFrame for QGraphicsViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QFrame>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QGraphicsView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QGraphicsViewPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QGraphicsView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQAbstractButton for QToolButtonPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QAbstractButton>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolButton>*"] -> (bool , QAbstractButton) as "std::tuple<bool, QAbstractButton*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractButton *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractButton * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QToolButtonPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolButton>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QFocusFramePtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QFocusFrame>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QGroupBoxPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QGroupBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QCalendarWidgetPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QCalendarWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
impl IQWidget for QMainWindowPtr {
    fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, QWidget>> {
        unsafe {
            let (is_safe, ptr) = cpp ! ([self as "QPointer<QMainWindow>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
            match is_safe {
                true => Some(QRef::new::<'a>(ptr)),
                false => None,
            }
        }
    }
}
