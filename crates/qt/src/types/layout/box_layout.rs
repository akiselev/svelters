use qt_widgets::{
    QBoxLayout, QDialog, QFrame, QGraphicsLayoutItem, QGroupBox, QLayout, QLayoutItem,
    QStackedWidget, QWidget,
};

use crate::inner::*;

qdefine!(|cls: QLayoutItem| {
    methods!(cls(this: Q<QLayoutItem>) {
        invalidate[0]() { this.invalidate() };
    });
    props!(cls(this: Q<QLayoutItem>) {
        // isEmpty: bool {
        //     this.is_empty()
        // };
    });
});

define!(|cls: QLayout| {
    let cls = cls
        .superclass::<Qt<QObject>>()?
        .superclass::<Q<QLayoutItem>>()?;
    methods!(cls(this: Qt<QLayoutItem>) {
        // show[0]() { this.show() };
        // hide[0]() { this.hide() };
        // resize[2](w: i32, h: i32) {
        //     this.resize_2a(w, h)
        // };
    });
    props!(cls(this: Qt<QLayout>) {
        // height: i32 {
        //     this.height()
        // };
    });
});

define!(|cls: QBoxLayout| {
    let cls = cls.superclass::<Qt<QLayout>>()?;
    methods!(cls(this: Qt<QBoxLayout>) {
        // show[0]() { this.show() };
        // hide[0]() { this.hide() };
        // resize[2](w: i32, h: i32) {
        //     this.resize_2a(w, h)
        // };
    });
    props!(cls(this: Qt<QBoxLayout>) {
        // height: i32 {
        //     this.height()
        // };
    });
});

define!(|cls: qt_widgets::QButtonGroup| {
    cls.superclass::<Qt<QObject>>()?;
});

define!(|cls: qt_widgets::QFormLayout| {
    cls.superclass::<Qt<QLayout>>()?;
});

define!(|cls: qt_widgets::QGridLayout| {
    cls.superclass::<Qt<QLayout>>()?;
});

define!(|cls: QGroupBox| {
    cls.superclass::<Qt<QWidget>>()?
        .as_element("qGroupBox", || unsafe { Ok(Qt::new(QGroupBox::new())) });
});

define!(|cls: qt_widgets::QHBoxLayout| {
    cls.superclass::<Qt<QBoxLayout>>()?;
});

qdefine!(|cls: qt_widgets::QSizePolicy| {
    // TODO: add props
});

qdefine!(|cls: qt_widgets::QSpacerItem| {
    cls.superclass::<Q<QLayoutItem>>()?;
});

define!(|cls: qt_widgets::QStackedLayout| {
    cls.superclass::<Qt<QLayout>>()?;
});

define!(|cls: QDialog| {
    cls.superclass::<Qt<QWidget>>()?
        .as_element("qDialog", || unsafe { Ok(Qt::new(QDialog::new_0a())) });
});

define!(|cls: QFrame| {
    cls.superclass::<Qt<QWidget>>()?
        .as_element("qFrame", || unsafe { Ok(Qt::new(QFrame::new_0a())) });
});

define!(|cls: QStackedWidget| {
    cls.superclass::<Qt<QFrame>>()?
        .as_element("qStackedWidget", || unsafe {
            Ok(Qt::new(QStackedWidget::new_0a()))
        });
});

define!(|cls: qt_widgets::QVBoxLayout| {
    cls.superclass::<Qt<QBoxLayout>>()?;
});

qdefine!(|cls: qt_widgets::QWidgetItem| {
    cls.superclass::<Q<QLayoutItem>>()?;
});
