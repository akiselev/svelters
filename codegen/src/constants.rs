use lazy_static::lazy_static;

lazy_static! {
    pub static ref PRIMITIVE_TYPES: Vec<&'static str> =
        vec!["bool", "double", "qreal", "int", "void"];
    pub static ref IMPLEMENTED_TYPES: Vec<&'static str> = vec![
        "QByteArray",
        "QColor",
        "QDate",
        "QDateTime",
        "QImage",
        "QMargins",
        "QModelIndex",
        // "QPixmap",
        "QPoint",
        "QPointF",
        "QRectF",
        "QSize",
        "QSizeF",
        "QString",
        "QTime",
        "QUrl",
        "QVariant",
        "QVariantList",
    ];
    pub static ref CODEGEN_IGNORE_CONSTRUCTOR: Vec<&'static str> =
        vec!["QSizeGrip", "QHeaderView", "QRubberBand", "QSplitterHandle",];
    pub static ref INHERITANCE_IGNORE_PARENT: Vec<&'static str> =
        vec!["QAccessibleObject", "QColorMap"];
}
