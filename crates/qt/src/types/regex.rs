/*
 * ---- PROPERTIES ----
 * from: ([a-z][a-zA-Z0-9]+) +: +([a-zA-Z]+)
 * to:     $1: $2 {\n            this.$1();\n            this.set_$2($1);\n        };
 */
/*
   autoExclusive : bool
   autoRepeat : bool
   autoRepeatDelay : int
   autoRepeatInterval : int
   checkable : bool
   checked : bool
   down : bool
   icon : QIcon
   iconSize : QSize
   shortcut : QKeySequence
   text : QString
*/

/*
 * ---- CLASSES ----
 * from: ([a-zA-Z0-9]+)\n\t\n+.+$
 * to:
define!(|cls: $1| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: $1) {

    });
    methods!(cls(this: $1) {

    });
});
 */
/*
define!(|cls: QAbstractGraphicsShapeItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QAbstractGraphicsShapeItem) {

    });
    methods!(cls(this: QAbstractGraphicsShapeItem) {

    });
});

define!(|cls: QGraphicsAnchor| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsAnchor) {

    });
    methods!(cls(this: QGraphicsAnchor) {

    });
});

define!(|cls: QGraphicsAnchorLayout| {
    let cls = cls.superclass::<Qt<QGraphicsLayout>>()?;
    props!(cls(this: QGraphicsAnchorLayout) {

    });
    methods!(cls(this: QGraphicsAnchorLayout) {

    });
});

define!(|cls: QGraphicsEffect| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsEffect) {

    });
    methods!(cls(this: QGraphicsEffect) {

    });
});

define!(|cls: QGraphicsEllipseItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QGraphicsEllipseItem) {

    });
    methods!(cls(this: QGraphicsEllipseItem) {

    });
});

define!(|cls: QGraphicsGridLayout| {
    let cls = cls.superclass::<Qt<QGraphicsLayout>>()?;
    props!(cls(this: QGraphicsGridLayout) {

    });
    methods!(cls(this: QGraphicsGridLayout) {

    });
});

define!(|cls: QGraphicsItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QGraphicsItem) {

    });
    methods!(cls(this: QGraphicsItem) {

    });
});

define!(|cls: QGraphicsItemGroup| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsItemGroup) {

    });
    methods!(cls(this: QGraphicsItemGroup) {

    });
});

define!(|cls: QGraphicsLayout| {
    let cls = cls.superclass::<Qt<QGraphicsLayout>>()?;
    props!(cls(this: QGraphicsLayout) {

    });
    methods!(cls(this: QGraphicsLayout) {

    });
});

define!(|cls: QGraphicsLayoutItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QGraphicsLayoutItem) {

    });
    methods!(cls(this: QGraphicsLayoutItem) {

    });
});

define!(|cls: QGraphicsLineItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QGraphicsLineItem) {

    });
    methods!(cls(this: QGraphicsLineItem) {

    });
});

define!(|cls: QGraphicsLinearLayout| {
    let cls = cls.superclass::<Qt<QGraphicsLayout>>()?;
    props!(cls(this: QGraphicsLinearLayout) {

    });
    methods!(cls(this: QGraphicsLinearLayout) {

    });
});

define!(|cls: QGraphicsObject| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsObject) {

    });
    methods!(cls(this: QGraphicsObject) {

    });
});

define!(|cls: QGraphicsPathItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QGraphicsPathItem) {

    });
    methods!(cls(this: QGraphicsPathItem) {

    });
});

define!(|cls: QGraphicsPixmapItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QGraphicsPixmapItem) {

    });
    methods!(cls(this: QGraphicsPixmapItem) {

    });
});

define!(|cls: QGraphicsPolygonItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QGraphicsPolygonItem) {

    });
    methods!(cls(this: QGraphicsPolygonItem) {

    });
});

define!(|cls: QGraphicsProxyWidget| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsProxyWidget) {

    });
    methods!(cls(this: QGraphicsProxyWidget) {

    });
});

define!(|cls: QGraphicsRectItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QGraphicsRectItem) {

    });
    methods!(cls(this: QGraphicsRectItem) {

    });
});

define!(|cls: QGraphicsScene| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsScene) {

    });
    methods!(cls(this: QGraphicsScene) {

    });
});

define!(|cls: QGraphicsSceneContextMenuEvent| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsSceneContextMenuEvent) {

    });
    methods!(cls(this: QGraphicsSceneContextMenuEvent) {

    });
});

define!(|cls: QGraphicsSceneDragDropEvent| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsSceneDragDropEvent) {

    });
    methods!(cls(this: QGraphicsSceneDragDropEvent) {

    });
});

define!(|cls: QGraphicsSceneEvent| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsSceneEvent) {

    });
    methods!(cls(this: QGraphicsSceneEvent) {

    });
});

define!(|cls: QGraphicsSceneHelpEvent| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsSceneHelpEvent) {

    });
    methods!(cls(this: QGraphicsSceneHelpEvent) {

    });
});

define!(|cls: QGraphicsSceneHoverEvent| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsSceneHoverEvent) {

    });
    methods!(cls(this: QGraphicsSceneHoverEvent) {

    });
});

define!(|cls: QGraphicsSceneMouseEvent| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsSceneMouseEvent) {

    });
    methods!(cls(this: QGraphicsSceneMouseEvent) {

    });
});

define!(|cls: QGraphicsSceneMoveEvent| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsSceneMoveEvent) {

    });
    methods!(cls(this: QGraphicsSceneMoveEvent) {

    });
});

define!(|cls: QGraphicsSceneResizeEvent| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsSceneResizeEvent) {

    });
    methods!(cls(this: QGraphicsSceneResizeEvent) {

    });
});

define!(|cls: QGraphicsSceneWheelEvent| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsSceneWheelEvent) {

    });
    methods!(cls(this: QGraphicsSceneWheelEvent) {

    });
});

define!(|cls: QGraphicsSimpleTextItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QGraphicsSimpleTextItem) {

    });
    methods!(cls(this: QGraphicsSimpleTextItem) {

    });
});

define!(|cls: QGraphicsSvgItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QGraphicsSvgItem) {

    });
    methods!(cls(this: QGraphicsSvgItem) {

    });
});

define!(|cls: QGraphicsTextItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QGraphicsTextItem) {

    });
    methods!(cls(this: QGraphicsTextItem) {

    });
});

define!(|cls: QGraphicsTransform| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsTransform) {

    });
    methods!(cls(this: QGraphicsTransform) {

    });
});

define!(|cls: QGraphicsView| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsView) {

    });
    methods!(cls(this: QGraphicsView) {

    });
});

define!(|cls: QGraphicsWidget| {
    let cls = cls.superclass::<Qt<QWidget>>()?;
    props!(cls(this: QGraphicsWidget) {

    });
    methods!(cls(this: QGraphicsWidget) {

    });
});

define!(|cls: QStyleOptionGraphicsItem| {
    let cls = cls.superclass::<Qt<QGraphicsItem>>()?;
    props!(cls(this: QStyleOptionGraphicsItem) {

    });
    methods!(cls(this: QStyleOptionGraphicsItem) {

    });
});
*/
