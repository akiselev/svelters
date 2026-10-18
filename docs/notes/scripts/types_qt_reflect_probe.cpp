// What Qt's own reflection gives for non-GUI types (docs/notes/types-research.md section 4).
//
// For value types: QMetaType traits (default/copy constructible, equality, a registered
// QString converter) and whether a gadget metaobject carries properties or invokables.
// For QObjects: how many members QMetaObject reaches (properties, signals, nslots, invokables)
// versus what the class only has as plain C++ methods (counted from qdoc, not here).
//
// Build/run (no moc needed):
//   g++ -std=c++17 -fPIC types_qt_reflect_probe.cpp $(pkg-config --cflags --libs Qt6Widgets) -o /tmp/qtprobe
//   QT_QPA_PLATFORM=offscreen /tmp/qtprobe
#include <QtCore>
#include <QtGui>
#include <QtWidgets>
#include <cstdio>

template <class T>
static void value(const char *name) {
  QMetaType mt = QMetaType::fromType<T>();
  const QMetaObject *mo = mt.metaObject();
  int props = 0, meths = 0, enums = 0;
  if (mo) {
    props = mo->propertyCount() - mo->propertyOffset();
    meths = mo->methodCount() - mo->methodOffset();
    enums = mo->enumeratorCount() - mo->enumeratorOffset();
  }
  bool toStr = QMetaType::canConvert(mt, QMetaType::fromType<QString>());
  bool fromStr = QMetaType::canConvert(QMetaType::fromType<QString>(), mt);
  std::printf("%-22s id=%-5d dflt=%d copy=%d eq=%d toStr=%d fromStr=%d gadget=%d props=%d invokables=%d enums=%d\n", name,
              mt.id(), mt.isDefaultConstructible(), mt.isCopyConstructible(), mt.isEqualityComparable(),
              toStr, fromStr, mo != nullptr, props, meths, enums);
}

static void object(QObject *o) {
  const QMetaObject *mo = o->metaObject();
  int props = 0, sigs = 0, nslots = 0, inv = 0, ctors = mo->constructorCount();
  // Count the whole chain up to (not including) QObject.
  for (const QMetaObject *m = mo; m && m != &QObject::staticMetaObject; m = m->superClass()) {
    props += m->propertyCount() - m->propertyOffset();
    for (int i = m->methodOffset(); i < m->methodCount(); ++i) {
      switch (m->method(i).methodType()) {
        case QMetaMethod::Signal: ++sigs; break;
        case QMetaMethod::Slot: ++nslots; break;
        default: ++inv; break;
      }
    }
  }
  std::printf("%-26s props=%-3d signals=%-3d nslots=%-3d invokables=%-3d ctors=%d\n", mo->className(), props, sigs, nslots, inv, ctors);
}

int main(int argc, char **argv) {
  QApplication app(argc, argv);
  std::printf("-- value types\n");
#define V(T) value<T>(#T)
  V(QPoint); V(QPointF); V(QSize); V(QRect); V(QRectF); V(QMargins); V(QLine); V(QColor);
  V(QUrl); V(QDate); V(QTime); V(QDateTime); V(QTimeZone); V(QLocale); V(QUuid); V(QByteArray);
  V(QStringList); V(QVariant); V(QVariantMap); V(QJsonValue); V(QJsonObject); V(QRegularExpression);
  V(QModelIndex); V(QPersistentModelIndex); V(QItemSelection); V(QMimeType); V(QEasingCurve);
  V(QFont); V(QIcon); V(QPixmap); V(QImage); V(QBrush); V(QPen); V(QPalette); V(QCursor);
  V(QKeySequence); V(QTransform); V(QPainterPath); V(QPolygonF); V(QRegion); V(QTextCharFormat);
  V(QTextCursor); V(QSizePolicy); V(QVector3D); V(QColorSpace); V(QPageLayout);
  std::printf("-- QObjects (QMetaObject reach, chain below QObject)\n");
  QTimer t; object(&t);
  QSettings s("svelters", "probe"); object(&s);
  QFile f; object(&f);
  QFileSystemWatcher w; object(&w);
  QStandardItemModel sm; object(&sm);
  QStringListModel slm; object(&slm);
  QSortFilterProxyModel pm; object(&pm);
  QItemSelectionModel ism(&sm); object(&ism);
  QFileSystemModel fsm; object(&fsm);
  QMimeData md; object(&md);
  object(QGuiApplication::clipboard());
  QAction a("x"); object(&a);
  QIntValidator v; object(&v);
  QCompleter c; object(&c);
  QTextDocument d; object(&d);
  QProcess p; object(&p);
  
  return 0;
}
