use qt_widgets::*;

define!(|cls: Qt<QAbstractButton>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QAbstractButton) {
        text: Q<QString> {
            Q::Box(this.text());
            this.set_text(text.to_ref())
        };
    });
    methods!(cls(this: QAbstractButton) {

    });
    event!(cls.clicked as SlotOfBool[val](val));
});

define!(|cls: Qt<QAbstractScrollArea>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QAbstractScrollArea) {

    });
    methods!(cls(this: QAbstractScrollArea) {

    });
});

define!(|cls: Qt<QAbstractSlider>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QAbstractSlider) {

    });
    methods!(cls(this: QAbstractSlider) {

    });
});

define!(|cls: Qt<QAbstractSpinBox>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QAbstractSpinBox) {

    });
    methods!(cls(this: QAbstractSpinBox) {

    });
});

define!(|cls: Qt<QCheckBox>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QCheckBox) {

    });
    methods!(cls(this: QCheckBox) {

    });
});

define!(|cls: Qt<QComboBox>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QComboBox) {

    });
    methods!(cls(this: QComboBox) {

    });
});

define!(|cls: Qt<QCommandLinkButton>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QCommandLinkButton) {

    });
    methods!(cls(this: QCommandLinkButton) {

    });
});

define!(|cls: Qt<QDateEdit>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QDateEdit) {

    });
    methods!(cls(this: QDateEdit) {

    });
});

define!(|cls: Qt<QDateTimeEdit>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QDateTimeEdit) {

    });
    methods!(cls(this: QDateTimeEdit) {

    });
});

define!(|cls: Qt<QDial>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QDial) {

    });
    methods!(cls(this: QDial) {

    });
});

define!(|cls: Qt<QDoubleSpinBox>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QDoubleSpinBox) {

    });
    methods!(cls(this: QDoubleSpinBox) {

    });
});

define!(|cls: Qt<QFocusFrame>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QFocusFrame) {

    });
    methods!(cls(this: QFocusFrame) {

    });
});

define!(|cls: Qt<QFontComboBox>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QFontComboBox) {

    });
    methods!(cls(this: QFontComboBox) {

    });
});

define!(|cls: Qt<QLCDNumber>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QLCDNumber) {

    });
    methods!(cls(this: QLCDNumber) {

    });
});

define!(|cls: Qt<QLabel>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QLabel) {

    });
    methods!(cls(this: QLabel) {

    });
});

define!(|cls: Qt<QLineEdit>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QLineEdit) {

    });
    methods!(cls(this: QLineEdit) {

    });
});

define!(|cls: Qt<QMenu>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QMenu) {

    });
    methods!(cls(this: QMenu) {

    });
});

define!(|cls: Qt<QProgressBar>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QProgressBar) {

    });
    methods!(cls(this: QProgressBar) {

    });
});

define!(|cls: Qt<QPushButton>| {
    let cls = cls
        .superclass::<Qt<QAbstractButton>>()?
        .constructor0(|| unsafe { Ok(Qt::new(QPushButton::new())) })?
        .as_element("qPushButton", || unsafe { Ok(Qt::new(QPushButton::new())) })?;
    props!(cls(this: QPushButton) {

    });
    methods!(cls(this: QPushButton) {

    });
});

define!(|cls: Qt<QRadioButton>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QRadioButton) {

    });
    methods!(cls(this: QRadioButton) {

    });
});

define!(|cls: Qt<QScrollArea>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QScrollArea) {

    });
    methods!(cls(this: QScrollArea) {

    });
});

define!(|cls: Qt<QScrollBar>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QScrollBar) {

    });
    methods!(cls(this: QScrollBar) {

    });
});

define!(|cls: Qt<QSizeGrip>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QSizeGrip) {

    });
    methods!(cls(this: QSizeGrip) {

    });
});

define!(|cls: Qt<QSlider>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QSlider) {

    });
    methods!(cls(this: QSlider) {

    });
});

define!(|cls: Qt<QSpinBox>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QSpinBox) {

    });
    methods!(cls(this: QSpinBox) {

    });
});

define!(|cls: Qt<QTabBar>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QTabBar) {

    });
    methods!(cls(this: QTabBar) {

    });
});

define!(|cls: Qt<QTabWidget>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QTabWidget) {

    });
    methods!(cls(this: QTabWidget) {

    });
});

define!(|cls: Qt<QTimeEdit>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QTimeEdit) {

    });
    methods!(cls(this: QTimeEdit) {

    });
});

define!(|cls: Qt<QToolBox>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QToolBox) {

    });
    methods!(cls(this: QToolBox) {

    });
});

define!(|cls: Qt<QToolButton>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QToolButton) {

    });
    methods!(cls(this: QToolButton) {

    });
});

define!(|cls: Qt<QWidget>| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QWidget) {

    });
    methods!(cls(this: QWidget) {

    });
});
