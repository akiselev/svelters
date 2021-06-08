use crate::{QObject, QObjectPtr, QWidget, QWidgetPtr};
use cast_trait_object::{dyn_cast, dyn_upcast, DynCastExt};
use cpp::{cpp, cpp_class};
use qttypes::*;

cpp! {{
#include <QtWidgets/QFontComboBox>
#include <QtWidgets/QProgressBar>
#include <QtWidgets/QOpenGLWidget>
#include <QtWidgets/QTextBrowser>
#include <QtWidgets/QAbstractSpinBox>
#include <QtWidgets/QProgressDialog>
#include <QtWidgets/QMenuBar>
#include <QtWidgets/QHeaderView>
#include <QtWidgets/QListWidget>
#include <QtWidgets/QMessageBox>
#include <QtWidgets/QSplitter>
#include <QtWidgets/QTimeEdit>
#include <QtWidgets/QDialogButtonBox>
#include <QtWidgets/QMenu>
#include <QtWidgets/QTabWidget>
#include <QtWidgets/QDialog>
#include <QtWidgets/QLCDNumber>
#include <QtWidgets/QPlainTextEdit>
#include <QtWidgets/QSpinBox>
#include <QtWidgets/QScrollBar>
#include <QtWidgets/QSlider>
#include <QtWidgets/QKeySequenceEdit>
#include <QtWidgets/QLabel>
#include <QtWidgets/QWizard>
#include <QtWidgets/QTextEdit>
#include <QtWidgets/QListView>
#include <QtWidgets/QToolBar>
#include <QtWidgets/QAbstractItemView>
#include <QtWidgets/QMdiArea>
#include <QtWidgets/QStackedWidget>
#include <QtWidgets/QInputDialog>
#include <QtWidgets/QSplashScreen>
#include <QtWidgets/QRadioButton>
#include <QtWidgets/QCalendarWidget>
#include <QtWidgets/QTableView>
#include <QtWidgets/QComboBox>
#include <QtWidgets/QPushButton>
#include <QtWidgets/QFocusFrame>
#include <QtWidgets/QDateEdit>
#include <QtWidgets/QStatusBar>
#include <QtWidgets/QDockWidget>
#include <QtWidgets/QLineEdit>
#include <QtWidgets/QUndoView>
#include <QtWidgets/QSizeGrip>
#include <QtWidgets/QDoubleSpinBox>
#include <QtWidgets/QScrollArea>
#include <QtWidgets/QDial>
#include <QtWidgets/QCommandLinkButton>
#include <QtWidgets/QAbstractSlider>
#include <QtWidgets/QGroupBox>
#include <QtWidgets/QGraphicsView>
#include <QtWidgets/QMdiSubWindow>
#include <QtWidgets/QTabBar>
#include <QtWidgets/QCheckBox>
#include <QtWidgets/QAbstractButton>
#include <QtWidgets/QSplitterHandle>
#include <QtWidgets/QTreeView>
#include <QtWidgets/QAbstractScrollArea>
#include <QtWidgets/QWizardPage>
#include <QtWidgets/QMainWindow>
#include <QtWidgets/QTreeWidget>
#include <QtWidgets/QToolBox>
#include <QtWidgets/QFrame>
#include <QtWidgets/QErrorMessage>
#include <QtWidgets/QColumnView>
#include <QtWidgets/QToolButton>
#include <QtWidgets/QRubberBand>
#include <QtWidgets/QFontDialog>
#include <QtWidgets/QTableWidget>
#include <QtWidgets/QFileDialog>
#include <QtWidgets/QColorDialog>
#include <QtWidgets/QDateTimeEdit>
#include <memory>
#include <tuple>
#include <QtCore/QPointer>
#include <QtCore/QObject>
}}

#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
)]
#[dyn_upcast]
pub trait IQObject {
    unsafe fn as_qt_ref(&self) -> Option<QObject>;
}
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QWidget>;
}
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
cpp_class ! (pub unsafe struct QSpinBoxPtr as "QPointer<QSpinBox>");
cpp_class ! (pub unsafe struct QSpinBox as "QSpinBox*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QSpinBox>;
}
cpp_class ! (pub unsafe struct QComboBoxPtr as "QPointer<QComboBox>");
cpp_class ! (pub unsafe struct QComboBox as "QComboBox*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QComboBox>;
}
cpp_class ! (pub unsafe struct QMdiSubWindowPtr as "QPointer<QMdiSubWindow>");
cpp_class ! (pub unsafe struct QMdiSubWindow as "QMdiSubWindow*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QMdiSubWindow>;
}
cpp_class ! (pub unsafe struct QSizeGripPtr as "QPointer<QSizeGrip>");
cpp_class ! (pub unsafe struct QSizeGrip as "QSizeGrip*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QSizeGrip>;
}
cpp_class ! (pub unsafe struct QMdiAreaPtr as "QPointer<QMdiArea>");
cpp_class ! (pub unsafe struct QMdiArea as "QMdiArea*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QMdiArea>;
}
cpp_class ! (pub unsafe struct QLCDNumberPtr as "QPointer<QLCDNumber>");
cpp_class ! (pub unsafe struct QLCDNumber as "QLCDNumber*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QLCDNumber>;
}
cpp_class ! (pub unsafe struct QTabBarPtr as "QPointer<QTabBar>");
cpp_class ! (pub unsafe struct QTabBar as "QTabBar*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QTabBar>;
}
cpp_class ! (pub unsafe struct QFontDialogPtr as "QPointer<QFontDialog>");
cpp_class ! (pub unsafe struct QFontDialog as "QFontDialog*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QFontDialog>;
}
cpp_class ! (pub unsafe struct QGraphicsViewPtr as "QPointer<QGraphicsView>");
cpp_class ! (pub unsafe struct QGraphicsView as "QGraphicsView*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QGraphicsView>;
}
cpp_class ! (pub unsafe struct QMenuPtr as "QPointer<QMenu>");
cpp_class ! (pub unsafe struct QMenu as "QMenu*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QMenu>;
}
cpp_class ! (pub unsafe struct QScrollBarPtr as "QPointer<QScrollBar>");
cpp_class ! (pub unsafe struct QScrollBar as "QScrollBar*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QScrollBar>;
}
cpp_class ! (pub unsafe struct QDockWidgetPtr as "QPointer<QDockWidget>");
cpp_class ! (pub unsafe struct QDockWidget as "QDockWidget*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QDockWidget>;
}
cpp_class ! (pub unsafe struct QKeySequenceEditPtr as "QPointer<QKeySequenceEdit>");
cpp_class ! (pub unsafe struct QKeySequenceEdit as "QKeySequenceEdit*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QKeySequenceEdit>;
}
cpp_class ! (pub unsafe struct QUndoViewPtr as "QPointer<QUndoView>");
cpp_class ! (pub unsafe struct QUndoView as "QUndoView*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QUndoView>;
}
cpp_class ! (pub unsafe struct QCheckBoxPtr as "QPointer<QCheckBox>");
cpp_class ! (pub unsafe struct QCheckBox as "QCheckBox*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QCheckBox>;
}
cpp_class ! (pub unsafe struct QStatusBarPtr as "QPointer<QStatusBar>");
cpp_class ! (pub unsafe struct QStatusBar as "QStatusBar*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QStatusBar>;
}
cpp_class ! (pub unsafe struct QTreeViewPtr as "QPointer<QTreeView>");
cpp_class ! (pub unsafe struct QTreeView as "QTreeView*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QTreeView>;
}
cpp_class ! (pub unsafe struct QInputDialogPtr as "QPointer<QInputDialog>");
cpp_class ! (pub unsafe struct QInputDialog as "QInputDialog*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QInputDialog>;
}
cpp_class ! (pub unsafe struct QDoubleSpinBoxPtr as "QPointer<QDoubleSpinBox>");
cpp_class ! (pub unsafe struct QDoubleSpinBox as "QDoubleSpinBox*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QDoubleSpinBox>;
}
cpp_class ! (pub unsafe struct QSplashScreenPtr as "QPointer<QSplashScreen>");
cpp_class ! (pub unsafe struct QSplashScreen as "QSplashScreen*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QSplashScreen>;
}
cpp_class ! (pub unsafe struct QRubberBandPtr as "QPointer<QRubberBand>");
cpp_class ! (pub unsafe struct QRubberBand as "QRubberBand*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QRubberBand>;
}
cpp_class ! (pub unsafe struct QListViewPtr as "QPointer<QListView>");
cpp_class ! (pub unsafe struct QListView as "QListView*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QListView>;
}
cpp_class ! (pub unsafe struct QFileDialogPtr as "QPointer<QFileDialog>");
cpp_class ! (pub unsafe struct QFileDialog as "QFileDialog*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QFileDialog>;
}
cpp_class ! (pub unsafe struct QToolBoxPtr as "QPointer<QToolBox>");
cpp_class ! (pub unsafe struct QToolBox as "QToolBox*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QToolBox>;
}
cpp_class ! (pub unsafe struct QDateTimeEditPtr as "QPointer<QDateTimeEdit>");
cpp_class ! (pub unsafe struct QDateTimeEdit as "QDateTimeEdit*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QDateTimeEdit>;
}
cpp_class ! (pub unsafe struct QCalendarWidgetPtr as "QPointer<QCalendarWidget>");
cpp_class ! (pub unsafe struct QCalendarWidget as "QCalendarWidget*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QCalendarWidget>;
}
cpp_class ! (pub unsafe struct QErrorMessagePtr as "QPointer<QErrorMessage>");
cpp_class ! (pub unsafe struct QErrorMessage as "QErrorMessage*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QErrorMessage>;
}
cpp_class ! (pub unsafe struct QFramePtr as "QPointer<QFrame>");
cpp_class ! (pub unsafe struct QFrame as "QFrame*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QFrame>;
}
cpp_class ! (pub unsafe struct QHeaderViewPtr as "QPointer<QHeaderView>");
cpp_class ! (pub unsafe struct QHeaderView as "QHeaderView*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QHeaderView>;
}
cpp_class ! (pub unsafe struct QTabWidgetPtr as "QPointer<QTabWidget>");
cpp_class ! (pub unsafe struct QTabWidget as "QTabWidget*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QTabWidget>;
}
cpp_class ! (pub unsafe struct QLineEditPtr as "QPointer<QLineEdit>");
cpp_class ! (pub unsafe struct QLineEdit as "QLineEdit*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QLineEdit>;
}
cpp_class ! (pub unsafe struct QAbstractScrollAreaPtr as "QPointer<QAbstractScrollArea>");
cpp_class ! (pub unsafe struct QAbstractScrollArea as "QAbstractScrollArea*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea>;
}
cpp_class ! (pub unsafe struct QSplitterHandlePtr as "QPointer<QSplitterHandle>");
cpp_class ! (pub unsafe struct QSplitterHandle as "QSplitterHandle*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QSplitterHandle>;
}
cpp_class ! (pub unsafe struct QAbstractButtonPtr as "QPointer<QAbstractButton>");
cpp_class ! (pub unsafe struct QAbstractButton as "QAbstractButton*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QAbstractButton>;
}
cpp_class ! (pub unsafe struct QColumnViewPtr as "QPointer<QColumnView>");
cpp_class ! (pub unsafe struct QColumnView as "QColumnView*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QColumnView>;
}
cpp_class ! (pub unsafe struct QMessageBoxPtr as "QPointer<QMessageBox>");
cpp_class ! (pub unsafe struct QMessageBox as "QMessageBox*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QMessageBox>;
}
cpp_class ! (pub unsafe struct QGroupBoxPtr as "QPointer<QGroupBox>");
cpp_class ! (pub unsafe struct QGroupBox as "QGroupBox*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QGroupBox>;
}
cpp_class ! (pub unsafe struct QWizardPtr as "QPointer<QWizard>");
cpp_class ! (pub unsafe struct QWizard as "QWizard*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QWizard>;
}
cpp_class ! (pub unsafe struct QScrollAreaPtr as "QPointer<QScrollArea>");
cpp_class ! (pub unsafe struct QScrollArea as "QScrollArea*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QScrollArea>;
}
cpp_class ! (pub unsafe struct QAbstractSpinBoxPtr as "QPointer<QAbstractSpinBox>");
cpp_class ! (pub unsafe struct QAbstractSpinBox as "QAbstractSpinBox*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QAbstractSpinBox>;
}
cpp_class ! (pub unsafe struct QPushButtonPtr as "QPointer<QPushButton>");
cpp_class ! (pub unsafe struct QPushButton as "QPushButton*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QPushButton>;
}
cpp_class ! (pub unsafe struct QColorDialogPtr as "QPointer<QColorDialog>");
cpp_class ! (pub unsafe struct QColorDialog as "QColorDialog*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QColorDialog>;
}
cpp_class ! (pub unsafe struct QSliderPtr as "QPointer<QSlider>");
cpp_class ! (pub unsafe struct QSlider as "QSlider*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QSlider>;
}
cpp_class ! (pub unsafe struct QListWidgetPtr as "QPointer<QListWidget>");
cpp_class ! (pub unsafe struct QListWidget as "QListWidget*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QListWidget>;
}
cpp_class ! (pub unsafe struct QDialogPtr as "QPointer<QDialog>");
cpp_class ! (pub unsafe struct QDialog as "QDialog*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QDialog>;
}
cpp_class ! (pub unsafe struct QOpenGLWidgetPtr as "QPointer<QOpenGLWidget>");
cpp_class ! (pub unsafe struct QOpenGLWidget as "QOpenGLWidget*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QOpenGLWidget>;
}
cpp_class ! (pub unsafe struct QDialogButtonBoxPtr as "QPointer<QDialogButtonBox>");
cpp_class ! (pub unsafe struct QDialogButtonBox as "QDialogButtonBox*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QDialogButtonBox>;
}
cpp_class ! (pub unsafe struct QToolButtonPtr as "QPointer<QToolButton>");
cpp_class ! (pub unsafe struct QToolButton as "QToolButton*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QToolButton>;
}
cpp_class ! (pub unsafe struct QAbstractItemViewPtr as "QPointer<QAbstractItemView>");
cpp_class ! (pub unsafe struct QAbstractItemView as "QAbstractItemView*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QAbstractItemView>;
}
cpp_class ! (pub unsafe struct QPlainTextEditPtr as "QPointer<QPlainTextEdit>");
cpp_class ! (pub unsafe struct QPlainTextEdit as "QPlainTextEdit*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QPlainTextEdit>;
}
cpp_class ! (pub unsafe struct QFocusFramePtr as "QPointer<QFocusFrame>");
cpp_class ! (pub unsafe struct QFocusFrame as "QFocusFrame*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QFocusFrame>;
}
cpp_class ! (pub unsafe struct QTextBrowserPtr as "QPointer<QTextBrowser>");
cpp_class ! (pub unsafe struct QTextBrowser as "QTextBrowser*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QTextBrowser>;
}
cpp_class ! (pub unsafe struct QDateEditPtr as "QPointer<QDateEdit>");
cpp_class ! (pub unsafe struct QDateEdit as "QDateEdit*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QDateEdit>;
}
cpp_class ! (pub unsafe struct QWizardPagePtr as "QPointer<QWizardPage>");
cpp_class ! (pub unsafe struct QWizardPage as "QWizardPage*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QWizardPage>;
}
cpp_class ! (pub unsafe struct QMainWindowPtr as "QPointer<QMainWindow>");
cpp_class ! (pub unsafe struct QMainWindow as "QMainWindow*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QMainWindow>;
}
cpp_class ! (pub unsafe struct QTableViewPtr as "QPointer<QTableView>");
cpp_class ! (pub unsafe struct QTableView as "QTableView*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QTableView>;
}
cpp_class ! (pub unsafe struct QTableWidgetPtr as "QPointer<QTableWidget>");
cpp_class ! (pub unsafe struct QTableWidget as "QTableWidget*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QTableWidget>;
}
cpp_class ! (pub unsafe struct QTextEditPtr as "QPointer<QTextEdit>");
cpp_class ! (pub unsafe struct QTextEdit as "QTextEdit*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QTextEdit>;
}
cpp_class ! (pub unsafe struct QRadioButtonPtr as "QPointer<QRadioButton>");
cpp_class ! (pub unsafe struct QRadioButton as "QRadioButton*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QRadioButton>;
}
cpp_class ! (pub unsafe struct QDialPtr as "QPointer<QDial>");
cpp_class ! (pub unsafe struct QDial as "QDial*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QDial>;
}
cpp_class ! (pub unsafe struct QMenuBarPtr as "QPointer<QMenuBar>");
cpp_class ! (pub unsafe struct QMenuBar as "QMenuBar*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QMenuBar>;
}
cpp_class ! (pub unsafe struct QStackedWidgetPtr as "QPointer<QStackedWidget>");
cpp_class ! (pub unsafe struct QStackedWidget as "QStackedWidget*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QStackedWidget>;
}
cpp_class ! (pub unsafe struct QAbstractSliderPtr as "QPointer<QAbstractSlider>");
cpp_class ! (pub unsafe struct QAbstractSlider as "QAbstractSlider*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QAbstractSlider>;
}
cpp_class ! (pub unsafe struct QProgressDialogPtr as "QPointer<QProgressDialog>");
cpp_class ! (pub unsafe struct QProgressDialog as "QProgressDialog*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QProgressDialog>;
}
cpp_class ! (pub unsafe struct QSplitterPtr as "QPointer<QSplitter>");
cpp_class ! (pub unsafe struct QSplitter as "QSplitter*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QSplitter>;
}
cpp_class ! (pub unsafe struct QFontComboBoxPtr as "QPointer<QFontComboBox>");
cpp_class ! (pub unsafe struct QFontComboBox as "QFontComboBox*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QFontComboBox>;
}
cpp_class ! (pub unsafe struct QTreeWidgetPtr as "QPointer<QTreeWidget>");
cpp_class ! (pub unsafe struct QTreeWidget as "QTreeWidget*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QTreeWidget>;
}
cpp_class ! (pub unsafe struct QLabelPtr as "QPointer<QLabel>");
cpp_class ! (pub unsafe struct QLabel as "QLabel*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QLabel>;
}
cpp_class ! (pub unsafe struct QCommandLinkButtonPtr as "QPointer<QCommandLinkButton>");
cpp_class ! (pub unsafe struct QCommandLinkButton as "QCommandLinkButton*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QCommandLinkButton>;
}
cpp_class ! (pub unsafe struct QToolBarPtr as "QPointer<QToolBar>");
cpp_class ! (pub unsafe struct QToolBar as "QToolBar*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QToolBar>;
}
cpp_class ! (pub unsafe struct QProgressBarPtr as "QPointer<QProgressBar>");
cpp_class ! (pub unsafe struct QProgressBar as "QProgressBar*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QProgressBar>;
}
cpp_class ! (pub unsafe struct QTimeEditPtr as "QPointer<QTimeEdit>");
cpp_class ! (pub unsafe struct QTimeEdit as "QTimeEdit*");
#[dyn_cast(
    IQSplitter,
    IQLabel,
    IQMessageBox,
    IQCheckBox,
    IQSizeGrip,
    IQMainWindow,
    IQAbstractSlider,
    IQTextBrowser,
    IQKeySequenceEdit,
    IQTextEdit,
    IQListWidget,
    IQSpinBox,
    IQDial,
    IQDateTimeEdit,
    IQWizardPage,
    IQSplashScreen,
    IQDialogButtonBox,
    IQMenuBar,
    IQPushButton,
    IQErrorMessage,
    IQMdiArea,
    IQToolButton,
    IQRubberBand,
    IQLCDNumber,
    IQFontDialog,
    IQWidget,
    IQTabWidget,
    IQSlider,
    IQProgressDialog,
    IQMenu,
    IQRadioButton,
    IQAbstractSpinBox,
    IQHeaderView,
    IQGroupBox,
    IQStatusBar,
    IQListView,
    IQDockWidget,
    IQTreeView,
    IQTreeWidget,
    IQColumnView,
    IQCalendarWidget,
    IQScrollArea,
    IQProgressBar,
    IQComboBox,
    IQSplitterHandle,
    IQTableWidget,
    IQAbstractItemView,
    IQFocusFrame,
    IQScrollBar,
    IQInputDialog,
    IQAbstractButton,
    IQAbstractScrollArea,
    IQFontComboBox,
    IQTimeEdit,
    IQGraphicsView,
    IQUndoView,
    IQMdiSubWindow,
    IQToolBar,
    IQDoubleSpinBox,
    IQFrame,
    IQDateEdit,
    IQCommandLinkButton,
    IQDialog,
    IQStackedWidget,
    IQColorDialog,
    IQTableView,
    IQToolBox,
    IQWizard,
    IQOpenGLWidget,
    IQPlainTextEdit,
    IQLineEdit,
    IQTabBar,
    IQFileDialog
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
    unsafe fn as_qt_ref(&self) -> Option<QTimeEdit>;
}
impl IQDialog for QMessageBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QDialog> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMessageBox>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QMessageBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMessageBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQComboBox for QFontComboBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QComboBox> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFontComboBox>*"] -> (bool , QComboBox) as "std::tuple<bool, QComboBox*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QComboBox *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QComboBox * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QFontComboBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFontComboBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractItemView for QHeaderViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractItemView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QHeaderView>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QHeaderViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QHeaderView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QHeaderViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QHeaderView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QHeaderViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QHeaderView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QSplashScreenPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSplashScreen>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QSplitterPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSplitter>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QSplitterPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSplitter>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QStatusBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QStatusBar>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QAbstractButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractButton>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQTextEdit for QTextBrowserPtr {
    unsafe fn as_qt_ref(&self) -> Option<QTextEdit> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextBrowser>*"] -> (bool , QTextEdit) as "std::tuple<bool, QTextEdit*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QTextEdit *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QTextEdit * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QTextBrowserPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextBrowser>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QTextBrowserPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextBrowser>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QTextBrowserPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextBrowser>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QProgressBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QProgressBar>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QAbstractSliderPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractSlider>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQDialog for QErrorMessagePtr {
    unsafe fn as_qt_ref(&self) -> Option<QDialog> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QErrorMessage>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QErrorMessagePtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QErrorMessage>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQDialog for QColorDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QDialog> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QColorDialog>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QColorDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QColorDialog>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QMdiSubWindowPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMdiSubWindow>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDialog>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QMdiAreaPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMdiArea>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QMdiAreaPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMdiArea>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QMdiAreaPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMdiArea>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QAbstractScrollAreaPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractScrollArea>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QAbstractScrollAreaPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractScrollArea>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQDialog for QFontDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QDialog> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFontDialog>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QFontDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFontDialog>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractSlider for QSliderPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractSlider> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSlider>*"] -> (bool , QAbstractSlider) as "std::tuple<bool, QAbstractSlider*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSlider *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSlider * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QSliderPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSlider>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QTextEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextEdit>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QTextEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextEdit>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QTextEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTextEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractSlider for QDialPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractSlider> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDial>*"] -> (bool , QAbstractSlider) as "std::tuple<bool, QAbstractSlider*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSlider *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSlider * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QDialPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDial>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQTableView for QTableWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QTableView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableWidget>*"] -> (bool , QTableView) as "std::tuple<bool, QTableView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QTableView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QTableView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractItemView for QTableWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractItemView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableWidget>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QTableWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableWidget>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QTableWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableWidget>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QTableWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QMenuBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMenuBar>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQListView for QListWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QListView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QListWidget>*"] -> (bool , QListView) as "std::tuple<bool, QListView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QListView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QListView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractItemView for QListWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractItemView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QListWidget>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QListWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QListWidget>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QListWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QListWidget>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QListWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QListWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QPlainTextEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QPlainTextEdit>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QPlainTextEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QPlainTextEdit>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QPlainTextEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QPlainTextEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QLabelPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QLabel>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QLabelPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QLabel>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QDockWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDockWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QScrollAreaPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QScrollArea>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QScrollAreaPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QScrollArea>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QScrollAreaPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QScrollArea>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractSpinBox for QSpinBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractSpinBox> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSpinBox>*"] -> (bool , QAbstractSpinBox) as "std::tuple<bool, QAbstractSpinBox*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSpinBox *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSpinBox * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QSpinBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSpinBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractSpinBox for QDateTimeEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractSpinBox> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDateTimeEdit>*"] -> (bool , QAbstractSpinBox) as "std::tuple<bool, QAbstractSpinBox*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSpinBox *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSpinBox * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QDateTimeEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDateTimeEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QAbstractItemViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractItemView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QAbstractItemViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractItemView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QAbstractItemViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractItemView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQListView for QUndoViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QListView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QUndoView>*"] -> (bool , QListView) as "std::tuple<bool, QListView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QListView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QListView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractItemView for QUndoViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractItemView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QUndoView>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QUndoViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QUndoView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QUndoViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QUndoView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QUndoViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QUndoView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractItemView for QColumnViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractItemView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QColumnView>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QColumnViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QColumnView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QColumnViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QColumnView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QColumnViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QColumnView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QMenuPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMenu>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractButton for QToolButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractButton> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolButton>*"] -> (bool , QAbstractButton) as "std::tuple<bool, QAbstractButton*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractButton *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractButton * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QToolButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolButton>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQDateTimeEdit for QTimeEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QDateTimeEdit> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTimeEdit>*"] -> (bool , QDateTimeEdit) as "std::tuple<bool, QDateTimeEdit*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDateTimeEdit *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDateTimeEdit * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractSpinBox for QTimeEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractSpinBox> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTimeEdit>*"] -> (bool , QAbstractSpinBox) as "std::tuple<bool, QAbstractSpinBox*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSpinBox *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSpinBox * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QTimeEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTimeEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QAbstractSpinBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QAbstractSpinBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQDialog for QWizardPtr {
    unsafe fn as_qt_ref(&self) -> Option<QDialog> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QWizard>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QWizardPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QWizard>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractButton for QRadioButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractButton> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QRadioButton>*"] -> (bool , QAbstractButton) as "std::tuple<bool, QAbstractButton*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractButton *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractButton * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QRadioButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QRadioButton>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQTreeView for QTreeWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QTreeView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeWidget>*"] -> (bool , QTreeView) as "std::tuple<bool, QTreeView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QTreeView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QTreeView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractItemView for QTreeWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractItemView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeWidget>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QTreeWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeWidget>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QTreeWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeWidget>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QTreeWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QMainWindowPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QMainWindow>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQDialog for QProgressDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QDialog> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QProgressDialog>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QProgressDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QProgressDialog>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QTabWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTabWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QStackedWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QStackedWidget>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QStackedWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QStackedWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QCalendarWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QCalendarWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QGroupBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QGroupBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QToolBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolBar>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QRubberBandPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QRubberBand>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractButton for QPushButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractButton> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QPushButton>*"] -> (bool , QAbstractButton) as "std::tuple<bool, QAbstractButton*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractButton *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractButton * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QPushButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QPushButton>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQDateTimeEdit for QDateEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QDateTimeEdit> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDateEdit>*"] -> (bool , QDateTimeEdit) as "std::tuple<bool, QDateTimeEdit*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDateTimeEdit *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDateTimeEdit * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractSpinBox for QDateEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractSpinBox> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDateEdit>*"] -> (bool , QAbstractSpinBox) as "std::tuple<bool, QAbstractSpinBox*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSpinBox *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSpinBox * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QDateEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDateEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QToolBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolBox>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QToolBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QToolBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QFramePtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFrame>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QComboBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QComboBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractSlider for QScrollBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractSlider> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QScrollBar>*"] -> (bool , QAbstractSlider) as "std::tuple<bool, QAbstractSlider*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSlider *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSlider * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QScrollBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QScrollBar>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QWizardPagePtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QWizardPage>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQDialog for QInputDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QDialog> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QInputDialog>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QInputDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QInputDialog>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractItemView for QTreeViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractItemView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeView>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QTreeViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QTreeViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QTreeViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTreeView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QDialogButtonBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDialogButtonBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractButton for QCheckBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractButton> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QCheckBox>*"] -> (bool , QAbstractButton) as "std::tuple<bool, QAbstractButton*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractButton *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractButton * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QCheckBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QCheckBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QSplitterHandlePtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSplitterHandle>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQPushButton for QCommandLinkButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QPushButton> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QCommandLinkButton>*"] -> (bool , QPushButton) as "std::tuple<bool, QPushButton*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QPushButton *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QPushButton * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractButton for QCommandLinkButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractButton> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QCommandLinkButton>*"] -> (bool , QAbstractButton) as "std::tuple<bool, QAbstractButton*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractButton *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractButton * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QCommandLinkButtonPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QCommandLinkButton>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QOpenGLWidgetPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QOpenGLWidget>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QFocusFramePtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFocusFrame>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QSizeGripPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QSizeGrip>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractItemView for QListViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractItemView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QListView>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QListViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QListView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QListViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QListView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QListViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QListView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QLineEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QLineEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QLCDNumberPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QLCDNumber>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QLCDNumberPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QLCDNumber>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractItemView for QTableViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractItemView> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableView>*"] -> (bool , QAbstractItemView) as "std::tuple<bool, QAbstractItemView*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractItemView *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractItemView * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QTableViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QTableViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QTableViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTableView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQDialog for QFileDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QDialog> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFileDialog>*"] -> (bool , QDialog) as "std::tuple<bool, QDialog*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QDialog *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QDialog * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QFileDialogPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QFileDialog>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QTabBarPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QTabBar>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractScrollArea for QGraphicsViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractScrollArea> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QGraphicsView>*"] -> (bool , QAbstractScrollArea) as "std::tuple<bool, QAbstractScrollArea*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractScrollArea *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractScrollArea * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQFrame for QGraphicsViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QFrame> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QGraphicsView>*"] -> (bool , QFrame) as "std::tuple<bool, QFrame*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QFrame *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QFrame * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QGraphicsViewPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QGraphicsView>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQAbstractSpinBox for QDoubleSpinBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QAbstractSpinBox> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDoubleSpinBox>*"] -> (bool , QAbstractSpinBox) as "std::tuple<bool, QAbstractSpinBox*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QAbstractSpinBox *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QAbstractSpinBox * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QDoubleSpinBoxPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QDoubleSpinBox>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
impl IQWidget for QKeySequenceEditPtr {
    unsafe fn as_qt_ref(&self) -> Option<QWidget> {
        let (is_safe, ptr) = cpp ! ([self as "QPointer<QKeySequenceEdit>*"] -> (bool , QWidget) as "std::tuple<bool, QWidget*>" { if (self -> isNull ()) { return std :: make_tuple (false , (QWidget *) NULL) ; } else { return std :: make_tuple (true , qobject_cast < QWidget * > (* self)) ; } });
        match is_safe {
            true => Some(ptr),
            false => None,
        }
    }
}
