use qt_core::{QString, SlotNoArgs, SlotOfBool};
use qt_widgets::*;

use crate::inner::*;

define!(|cls: QAbstractButton| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QAbstractButton>) {
        text: Q<QString> {
            Q::Box(this.text());
            this.set_text(text.to_ref())
        };
        autoExclusive: bool {
            this.auto_exclusive();
            this.set_auto_exclusive(autoExclusive);
        };
        autoRepeat: bool {
            this.auto_repeat();
            this.set_auto_repeat(autoRepeat);
        };
        autoRepeatDelay: i32 {
            this.auto_repeat_delay();
            this.set_auto_repeat_delay(autoRepeatDelay);
        };
        autoRepeatInterval: i32 {
            this.auto_repeat_interval();
            this.set_auto_repeat_interval(autoRepeatInterval);
        };
        checkable: bool {
            this.is_checkable();
            this.set_checkable(checkable);
        };
        checked: bool {
            this.is_checked();
            this.set_checked(checked);
        };
        down: bool {
            this.is_down();
            this.set_down(down);
        };
        // icon: QIcon {
        //     this.icon();
        //     this.set_icon(icon);
        // };
        // iconSize: QSize {
        //     this.icon_size();
        //     this.set_icon_size(iconSize);
        // };
        // shortcut: QKeySequence {
        //     this.shortcut();
        //     this.set_shortcut(shortcut);
        // };
    });
    methods!(cls(this: Qt<QAbstractButton>) {

    });
    event!(cls.clicked as SlotOfBool[val](val));
    event!(cls.toggled as SlotOfBool[val](val));
    event!(cls.pressed as SlotNoArgs[]());
    event!(cls.released as SlotNoArgs[]());
});

define!(|cls: QAbstractScrollArea| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QAbstractScrollArea>) {

    });
    methods!(cls(this: Qt<QAbstractScrollArea>) {

    });
});

define!(|cls: QAbstractSlider| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QAbstractSlider>) {

    });
    methods!(cls(this: Qt<QAbstractSlider>) {

    });
});

define!(|cls: QAbstractSpinBox| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QAbstractSpinBox>) {

    });
    methods!(cls(this: Qt<QAbstractSpinBox>) {

    });
});

define!(|cls: QCheckBox| {
    let cls = cls
        .superclass::<Qt<QAbstractButton>>()?
        .constructor0(|| unsafe { Ok(Qt::new(QCheckBox::new())) })?
        .as_element("qCheckBox", || unsafe { Ok(Qt::new(QCheckBox::new())) })?;
    props!(cls(this: Qt<QCheckBox>) {

    });
    methods!(cls(this: Qt<QCheckBox>) {

    });
});

define!(|cls: QComboBox| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QComboBox>) {

    });
    methods!(cls(this: Qt<QComboBox>) {

    });
});

define!(|cls: QCommandLinkButton| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QCommandLinkButton>) {

    });
    methods!(cls(this: Qt<QCommandLinkButton>) {

    });
});

define!(|cls: QDateEdit| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QDateEdit>) {

    });
    methods!(cls(this: Qt<QDateEdit>) {

    });
});

define!(|cls: QDateTimeEdit| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QDateTimeEdit>) {

    });
    methods!(cls(this: Qt<QDateTimeEdit>) {

    });
});

define!(|cls: QDial| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QDial>) {

    });
    methods!(cls(this: Qt<QDial>) {

    });
});

define!(|cls: QDoubleSpinBox| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QDoubleSpinBox>) {

    });
    methods!(cls(this: Qt<QDoubleSpinBox>) {

    });
});

define!(|cls: QFocusFrame| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QFocusFrame>) {

    });
    methods!(cls(this: Qt<QFocusFrame>) {

    });
});

define!(|cls: QFontComboBox| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QFontComboBox>) {

    });
    methods!(cls(this: Qt<QFontComboBox>) {

    });
});

define!(|cls: QLCDNumber| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QLCDNumber>) {

    });
    methods!(cls(this: Qt<QLCDNumber>) {

    });
});

define!(|cls: QLabel| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QLabel>) {

    });
    methods!(cls(this: Qt<QLabel>) {

    });
});

define!(|cls: QLineEdit| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QLineEdit>) {

    });
    methods!(cls(this: Qt<QLineEdit>) {

    });
});

define!(|cls: QMenu| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QMenu>) {

    });
    methods!(cls(this: Qt<QMenu>) {

    });
});

define!(|cls: QProgressBar| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QProgressBar>) {

    });
    methods!(cls(this: Qt<QProgressBar>) {

    });
});

define!(|cls: QPushButton| {
    let cls = cls
        .superclass::<Qt<QAbstractButton>>()?
        .constructor0(|| unsafe { Ok(Qt::new(QPushButton::new())) })?
        .as_element("qPushButton", || unsafe { Ok(Qt::new(QPushButton::new())) })?;
    props!(cls(this: Qt<QPushButton>) {

    });
    methods!(cls(this: Qt<QPushButton>) {

    });
});

define!(|cls: QRadioButton| {
    let cls = cls
        .superclass::<Qt<QAbstractButton>>()?
        .constructor0(|| unsafe { Ok(Qt::new(QRadioButton::new())) })?
        .as_element("qRadioButton", || unsafe {
            Ok(Qt::new(QRadioButton::new()))
        })?;
    props!(cls(this: Qt<QRadioButton>) {

    });
    methods!(cls(this: Qt<QRadioButton>) {

    });
});

define!(|cls: QScrollArea| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QScrollArea>) {

    });
    methods!(cls(this: Qt<QScrollArea>) {

    });
});

define!(|cls: QScrollBar| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QScrollBar>) {

    });
    methods!(cls(this: Qt<QScrollBar>) {

    });
});

define!(|cls: QSizeGrip| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QSizeGrip>) {

    });
    methods!(cls(this: Qt<QSizeGrip>) {

    });
});

define!(|cls: QSlider| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QSlider>) {

    });
    methods!(cls(this: Qt<QSlider>) {

    });
});

define!(|cls: QSpinBox| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QSpinBox>) {

    });
    methods!(cls(this: Qt<QSpinBox>) {

    });
});

define!(|cls: QTabBar| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QTabBar>) {

    });
    methods!(cls(this: Qt<QTabBar>) {

    });
});

define!(|cls: QTabWidget| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QTabWidget>) {

    });
    methods!(cls(this: Qt<QTabWidget>) {

    });
});

define!(|cls: QTimeEdit| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QTimeEdit>) {

    });
    methods!(cls(this: Qt<QTimeEdit>) {

    });
});

define!(|cls: QToolBox| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QToolBox>) {

    });
    methods!(cls(this: Qt<QToolBox>) {

    });
});

define!(|cls: QToolButton| {
    let cls = cls
        .superclass::<Qt<QAbstractButton>>()?
        .constructor0(|| unsafe { Ok(Qt::new(QToolButton::new_0a())) })?
        .as_element("qToolButton", || unsafe {
            Ok(Qt::new(QToolButton::new_0a()))
        })?;
    props!(cls(this: Qt<QToolButton>) {

    });
    methods!(cls(this: Qt<QToolButton>) {

    });
});
