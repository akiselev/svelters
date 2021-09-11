use qt_core::QEvent;
use qt_widgets::*;

use crate::inner::*;

qdefine!(|cls: QAbstractGraphicsShapeItem| {
    let cls = cls.superclass::<Q<QGraphicsItem>>()?;
    props!(cls(this: Q<QAbstractGraphicsShapeItem>) {

    });
    methods!(cls(this: Q<QAbstractGraphicsShapeItem>) {

    });
});

define!(|cls: QGraphicsAnchor| {
    let cls = cls.superclass::<Qt<QObject>>()?;
    props!(cls(this: Qt<QGraphicsAnchor>) {

    });
    methods!(cls(this: Qt<QGraphicsAnchor>) {

    });
});

qdefine!(|cls: QGraphicsAnchorLayout| {
    let cls = cls.superclass::<Q<QGraphicsLayout>>()?;
    props!(cls(this: Q<QGraphicsAnchorLayout>) {

    });
    methods!(cls(this: Q<QGraphicsAnchorLayout>) {

    });
});

define!(|cls: QGraphicsEffect| {
    let cls = cls.superclass::<Qt<QObject>>()?;
    props!(cls(this: Qt<QGraphicsEffect>) {

    });
    methods!(cls(this: Qt<QGraphicsEffect>) {

    });
});

qdefine!(|cls: QGraphicsEllipseItem| {
    let cls = cls.superclass::<Q<QGraphicsItem>>()?;
    props!(cls(this: Q<QGraphicsEllipseItem>) {

    });
    methods!(cls(this: Q<QGraphicsEllipseItem>) {

    });
});

qdefine!(|cls: QGraphicsGridLayout| {
    let cls = cls.superclass::<Q<QGraphicsLayout>>()?;
    props!(cls(this: Q<QGraphicsGridLayout>) {

    });
    methods!(cls(this: Q<QGraphicsGridLayout>) {

    });
});

qdefine!(|cls: QGraphicsItem| {
    props!(cls(this: Q<QGraphicsItem>) {

    });
    methods!(cls(this: Q<QGraphicsItem>) {

    });
});

qdefine!(|cls: QGraphicsItemGroup| {
    let cls = cls.superclass::<Q<QGraphicsItem>>()?;
    props!(cls(this: Q<QGraphicsItemGroup>) {

    });
    methods!(cls(this: Q<QGraphicsItemGroup>) {

    });
});

qdefine!(|cls: QGraphicsLayout| {
    let cls = cls.superclass::<Q<QGraphicsLayoutItem>>()?;
    props!(cls(this: Q<QGraphicsLayout>) {

    });
    methods!(cls(this: Q<QGraphicsLayout>) {

    });
});

qdefine!(|cls: QGraphicsLayoutItem| {
    props!(cls(this: Q<QGraphicsLayoutItem>) {

    });
    methods!(cls(this: Q<QGraphicsLayoutItem>) {

    });
});

qdefine!(|cls: QGraphicsLineItem| {
    let cls = cls.superclass::<Q<QGraphicsItem>>()?;
    props!(cls(this: Q<QGraphicsLineItem>) {

    });
    methods!(cls(this: Q<QGraphicsLineItem>) {

    });
});

qdefine!(|cls: QGraphicsLinearLayout| {
    let cls = cls.superclass::<Q<QGraphicsLayout>>()?;
    props!(cls(this: Q<QGraphicsLinearLayout>) {

    });
    methods!(cls(this: Q<QGraphicsLinearLayout>) {

    });
});

define!(|cls: QGraphicsObject| {
    let cls = cls
        .superclass::<Qt<QObject>>()?
        .superclass::<Q<QGraphicsItem>>()?;
    props!(cls(this: Qt<QGraphicsObject>) {

    });
    methods!(cls(this: Qt<QGraphicsObject>) {

    });
});

qdefine!(|cls: QGraphicsPathItem| {
    let cls = cls.superclass::<Q<QGraphicsItem>>()?;
    props!(cls(this: Q<QGraphicsPathItem>) {

    });
    methods!(cls(this: Q<QGraphicsPathItem>) {

    });
});

qdefine!(|cls: QGraphicsPixmapItem| {
    let cls = cls.superclass::<Q<QGraphicsItem>>()?;
    props!(cls(this: Q<QGraphicsPixmapItem>) {

    });
    methods!(cls(this: Q<QGraphicsPixmapItem>) {

    });
});

qdefine!(|cls: QGraphicsPolygonItem| {
    let cls = cls.superclass::<Q<QGraphicsItem>>()?;
    props!(cls(this: Q<QGraphicsPolygonItem>) {

    });
    methods!(cls(this: Q<QGraphicsPolygonItem>) {

    });
});

define!(|cls: QGraphicsProxyWidget| {
    let cls = cls.superclass::<Qt<QGraphicsWidget>>()?;
    props!(cls(this: Qt<QGraphicsProxyWidget>) {

    });
    methods!(cls(this: Qt<QGraphicsProxyWidget>) {

    });
});

qdefine!(|cls: QGraphicsRectItem| {
    let cls = cls.superclass::<Q<QGraphicsItem>>()?;
    props!(cls(this: Q<QGraphicsRectItem>) {

    });
    methods!(cls(this: Q<QGraphicsRectItem>) {

    });
});

define!(|cls: QGraphicsScene| {
    let cls = cls.superclass::<Qt<QObject>>()?;
    props!(cls(this: Qt<QGraphicsScene>) {

    });
    methods!(cls(this: Qt<QGraphicsScene>) {

    });
});

qdefine!(|cls: QGraphicsSceneContextMenuEvent| {
    let cls = cls.superclass::<Q<QGraphicsSceneEvent>>()?;
    props!(cls(this: Q<QGraphicsSceneContextMenuEvent>) {

    });
    methods!(cls(this: Q<QGraphicsSceneContextMenuEvent>) {

    });
});

qdefine!(|cls: QGraphicsSceneDragDropEvent| {
    let cls = cls.superclass::<Q<QGraphicsSceneEvent>>()?;
    props!(cls(this: Q<QGraphicsSceneDragDropEvent>) {

    });
    methods!(cls(this: Q<QGraphicsSceneDragDropEvent>) {

    });
});

qdefine!(|cls: QGraphicsSceneEvent| {
    let cls = cls.superclass::<Q<QEvent>>()?;
    props!(cls(this: Q<QGraphicsSceneEvent>) {

    });
    methods!(cls(this: Q<QGraphicsSceneEvent>) {

    });
});

qdefine!(|cls: QGraphicsSceneHelpEvent| {
    let cls = cls.superclass::<Q<QGraphicsSceneEvent>>()?;
    props!(cls(this: Q<QGraphicsSceneHelpEvent>) {

    });
    methods!(cls(this: Q<QGraphicsSceneHelpEvent>) {

    });
});

qdefine!(|cls: QGraphicsSceneHoverEvent| {
    let cls = cls.superclass::<Q<QGraphicsSceneEvent>>()?;
    props!(cls(this: Q<QGraphicsSceneHoverEvent>) {

    });
    methods!(cls(this: Q<QGraphicsSceneHoverEvent>) {

    });
});

qdefine!(|cls: QGraphicsSceneMouseEvent| {
    let cls = cls.superclass::<Q<QGraphicsSceneEvent>>()?;
    props!(cls(this: Q<QGraphicsSceneMouseEvent>) {

    });
    methods!(cls(this: Q<QGraphicsSceneMouseEvent>) {

    });
});

qdefine!(|cls: QGraphicsSceneMoveEvent| {
    let cls = cls.superclass::<Q<QGraphicsSceneEvent>>()?;
    props!(cls(this: Q<QGraphicsSceneMoveEvent>) {

    });
    methods!(cls(this: Q<QGraphicsSceneMoveEvent>) {

    });
});

qdefine!(|cls: QGraphicsSceneResizeEvent| {
    let cls = cls.superclass::<Q<QGraphicsSceneEvent>>()?;
    props!(cls(this: Q<QGraphicsSceneResizeEvent>) {

    });
    methods!(cls(this: Q<QGraphicsSceneResizeEvent>) {

    });
});

qdefine!(|cls: QGraphicsSceneWheelEvent| {
    let cls = cls.superclass::<Q<QGraphicsSceneEvent>>()?;
    props!(cls(this: Q<QGraphicsSceneWheelEvent>) {

    });
    methods!(cls(this: Q<QGraphicsSceneWheelEvent>) {

    });
});

qdefine!(|cls: QGraphicsSimpleTextItem| {
    let cls = cls.superclass::<Q<QGraphicsItem>>()?;
    props!(cls(this: Q<QGraphicsSimpleTextItem>) {

    });
    methods!(cls(this: Q<QGraphicsSimpleTextItem>) {

    });
});

qdefine!(|cls: QGraphicsTextItem| {
    let cls = cls.superclass::<Q<QGraphicsItem>>()?;
    props!(cls(this: Q<QGraphicsTextItem>) {

    });
    methods!(cls(this: Q<QGraphicsTextItem>) {

    });
});

define!(|cls: QGraphicsTransform| {
    let cls = cls.superclass::<Qt<QObject>>()?;
    props!(cls(this: Qt<QGraphicsTransform>) {

    });
    methods!(cls(this: Qt<QGraphicsTransform>) {

    });
});

define!(|cls: QGraphicsView| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: Qt<QGraphicsView>) {

    });
    methods!(cls(this: Qt<QGraphicsView>) {

    });
});

define!(|cls: QGraphicsWidget| {
    let cls = cls
        .superclass::<Qt<QGraphicsObject>>()?
        .superclass::<Q<QGraphicsLayoutItem>>()?;
    props!(cls(this: Qt<QGraphicsWidget>) {

    });
    methods!(cls(this: Qt<QGraphicsWidget>) {

    });
});

qdefine!(|cls: QStyleOptionGraphicsItem| {
    let cls = cls.superclass::<Q<QStyleOption>>()?;
    props!(cls(this: Q<QStyleOptionGraphicsItem>) {

    });
    methods!(cls(this: Q<QStyleOptionGraphicsItem>) {

    });
});
