// Observability probe for Qt 6 (plans/research/REACTIVITY.md section 3.2).
//
// Part A (sweep): for each class below, every own (non-inherited) writable property of a scalar
// type (bool, int, double, QString, enum) is set to a new value and then to the same value again.
// Recorded per property: NOTIFY signal emissions (QSignalSpy), QBindable notifier calls
// (QMetaProperty::bindable + addNotifier), and the QEvent types an event filter saw.
// Output: one TSV row per property:
//   class prop has_notify bindable readback notify_change notify_same bindable_change events_change
// Part B (scenarios): user-style input (QTest key clicks), parent-driven changes, clamping, and
// property update groups.
//
// Build: g++ -std=c++17 -fPIC qt_observe_probe.cpp $(pkg-config --cflags --libs Qt6Widgets Qt6Test) -o /tmp/qt_observe_probe
// Run:   QT_QPA_PLATFORM=offscreen /tmp/qt_observe_probe > rows.tsv
#include <QtWidgets>
#include <QtTest/QSignalSpy>
#include <QtTest/QTest>
#include <cstdio>
#include <functional>

static const QMetaEnum kEventEnum = QMetaEnum::fromType<QEvent::Type>();

struct Filter : QObject {
  QStringList seen;
  bool eventFilter(QObject *, QEvent *e) override {
    switch (e->type()) {
    case QEvent::Paint: case QEvent::UpdateRequest: case QEvent::UpdateLater: case QEvent::Timer:
    case QEvent::MetaCall: case QEvent::DeferredDelete: case QEvent::ChildPolished: case QEvent::Polish:
    case QEvent::PolishRequest: case QEvent::ChildAdded: case QEvent::ChildRemoved: case QEvent::LayoutRequest:
      return false;
    default: break;
    }
    const char *k = kEventEnum.valueToKey(e->type());
    QString n = k ? QString::fromLatin1(k) : QString::number(e->type());
    if (!seen.contains(n)) seen << n;
    return false;
  }
};

static bool nextValue(const QMetaProperty &p, const QVariant &cur, QVariant &out) {
  if (p.isEnumType()) {
    QMetaEnum me = p.enumerator();
    int c = cur.toInt();
    for (int i = 0; i < me.keyCount(); i++)
      if (me.value(i) != c) { out = QVariant(me.value(i)); return true; }
    return false;
  }
  switch (p.metaType().id()) {
  case QMetaType::Bool: out = !cur.toBool(); return true;
  case QMetaType::Int: out = cur.toInt() + 1; return true;
  case QMetaType::Double: out = cur.toDouble() + 1.0; return true;
  case QMetaType::QString: out = cur.toString() + "x"; return true;
  default: return false;
  }
}

static void sweep(QObject *o, const char *label) {
  const QMetaObject *mo = o->metaObject();
  Filter f;
  o->installEventFilter(&f);
  for (int i = mo->propertyOffset(); i < mo->propertyCount(); i++) {
    QMetaProperty p = mo->property(i);
    if (!p.isWritable() || !p.isReadable()) continue;
    QVariant cur = p.read(o), nv;
    if (!nextValue(p, cur, nv)) continue;
    std::unique_ptr<QSignalSpy> spy;
    if (p.hasNotifySignal()) spy.reset(new QSignalSpy(o, p.notifySignal()));
    int bcount = 0;
    QPropertyNotifier notifier;
    QUntypedBindable b;
    if (p.isBindable()) {
      b = p.bindable(o);
      notifier = b.addNotifier([&] { bcount++; });
    }
    f.seen.clear();
    p.write(o, nv);
    QCoreApplication::processEvents();
    QVariant after = p.read(o);
    int nchange = spy ? spy->count() : -1, bchange = bcount;
    QStringList ev = f.seen;
    const char *rb = after == nv ? "exact" : after == cur ? "ignored" : "normalized";
    if (spy) spy->clear();
    bcount = 0;
    p.write(o, after);
    QCoreApplication::processEvents();
    int nsame = spy ? spy->count() : -1;
    printf("%s\t%s\t%d\t%d\t%s\t%d\t%d\t%d\t%s\n", label, p.name(), p.hasNotifySignal() ? 1 : 0, p.isBindable() ? 1 : 0, rb,
           nchange, nsame, p.isBindable() ? bchange : -1, qPrintable(ev.join(",")));
  }
  o->removeEventFilter(&f);
}

template <class T> static void widget(const char *name) {
  T *w = new T;
  w->resize(200, 100);
  w->show();
  QCoreApplication::processEvents();
  sweep(w, name);
  // QWidget's own props, observed through this class (inherited, so sweep() skipped them)
  delete w;
}

static void scenario(const char *what, const QString &result) { printf("#scenario\t%s\t%s\n", what, qPrintable(result)); }

int main(int argc, char **argv) {
  setvbuf(stdout, nullptr, _IOLBF, 0);
  QApplication app(argc, argv);
  printf("#qt\t%s\n", qVersion());

  // QWidget's own props on a plain shown QWidget
  {
    QWidget *w = new QWidget; w->resize(200, 100); w->show(); QCoreApplication::processEvents();
    sweep(w, "QWidget");
    delete w;
  }
  widget<QLineEdit>("QLineEdit");
  widget<QSpinBox>("QSpinBox");
  widget<QDoubleSpinBox>("QDoubleSpinBox");
  widget<QSlider>("QSlider");
  widget<QScrollBar>("QScrollBar");
  widget<QDial>("QDial");
  widget<QCheckBox>("QCheckBox");
  widget<QPushButton>("QPushButton");
  widget<QRadioButton>("QRadioButton");
  widget<QComboBox>("QComboBox");
  widget<QLabel>("QLabel");
  widget<QProgressBar>("QProgressBar");
  widget<QTextEdit>("QTextEdit");
  widget<QPlainTextEdit>("QPlainTextEdit");
  widget<QTabWidget>("QTabWidget");
  widget<QGroupBox>("QGroupBox");
  widget<QFrame>("QFrame");
  widget<QDateTimeEdit>("QDateTimeEdit");
  widget<QCalendarWidget>("QCalendarWidget");
  widget<QMainWindow>("QMainWindow");
  widget<QDialog>("QDialog");
  widget<QToolButton>("QToolButton");
  widget<QLCDNumber>("QLCDNumber");
  widget<QSplitter>("QSplitter");
  widget<QStackedWidget>("QStackedWidget");
  widget<QScrollArea>("QScrollArea");
  widget<QListWidget>("QListWidget");
  widget<QKeySequenceEdit>("QKeySequenceEdit");
  { QTimer t; sweep(&t, "QTimer"); }
  { QAction a(nullptr); sweep(&a, "QAction"); }
  { QSortFilterProxyModel m; sweep(&m, "QSortFilterProxyModel"); }
  { QTextDocument d; sweep(&d, "QTextDocument"); }
  { QPropertyAnimation a; sweep(&a, "QPropertyAnimation"); }
  { QStandardItemModel m; sweep(&m, "QStandardItemModel"); }

  // ---- Part B ----
  {
    QLineEdit e; e.show();
    QSignalSpy changed(&e, &QLineEdit::textChanged), edited(&e, &QLineEdit::textEdited);
    e.setText("ab"); e.setText("ab");
    scenario("QLineEdit setText ab,ab", QString("textChanged=%1 textEdited=%2").arg(changed.count()).arg(edited.count()));
    changed.clear(); edited.clear();
    e.setFocus(); QTest::keyClicks(&e, "xyz");
    scenario("QLineEdit keyClicks xyz", QString("textChanged=%1 textEdited=%2").arg(changed.count()).arg(edited.count()));
  }
  {
    QSlider s; s.setRange(0, 100); s.setValue(80);
    QSignalSpy vc(&s, &QAbstractSlider::valueChanged);
    s.setMaximum(50);
    scenario("QSlider setMaximum 50 with value 80", QString("valueChanged=%1 value=%2").arg(vc.count()).arg(s.value()));
  }
  {
    QWidget parent; QWidget *child = new QWidget(&parent); parent.show(); QCoreApplication::processEvents();
    Filter f; child->installEventFilter(&f);
    parent.setEnabled(false); QCoreApplication::processEvents();
    scenario("parent setEnabled(false): child events", f.seen.join(",") + QString(" child.isEnabled=%1").arg(child->isEnabled()));
    f.seen.clear();
    parent.hide(); QCoreApplication::processEvents();
    scenario("parent hide(): child events", f.seen.join(",") + QString(" child.isVisible=%1").arg(child->isVisible()));
  }
  {
    QWidget w; Filter f; w.installEventFilter(&f);
    w.resize(300, 200); QCoreApplication::processEvents();
    scenario("hidden QWidget resize: events", f.seen.join(",") + " (Resize pending until show)");
    f.seen.clear(); w.show(); QCoreApplication::processEvents();
    scenario("then show: events", f.seen.join(","));
  }
  {
    QTimer t; int n = 0;
    QPropertyNotifier pn = t.bindableInterval().addNotifier([&] { n++; });
    t.setInterval(10); t.setInterval(10); t.setInterval(20);
    scenario("QTimer setInterval 10,10,20: bindable notifier", QString::number(n));
    n = 0;
    Qt::beginPropertyUpdateGroup();
    t.setInterval(30); t.setInterval(40); t.setInterval(50);
    int inside = n;
    Qt::endPropertyUpdateGroup();
    scenario("QTimer 3 sets inside a property update group", QString("inside=%1 after=%2").arg(inside).arg(n));
    n = 0;
    t.start(); t.stop();
    QPropertyNotifier pa = t.bindableActive().addNotifier([&] { n++; });
    t.start(); t.stop();
    scenario("QTimer active start/stop: bindable notifier", QString::number(n));
  }
  {
    QSpinBox sb; sb.setRange(0, 99);
    QSignalSpy vc(&sb, qOverload<int>(&QSpinBox::valueChanged));
    sb.setValue(500);
    scenario("QSpinBox setValue 500 (max 99)", QString("valueChanged=%1 value=%2").arg(vc.count()).arg(sb.value()));
  }
  {
    // timing: NOTIFY + read-back vs event filter cost, 100k programmatic sets
    QSlider s; s.setRange(0, 1000000);
    double sink = 0;
    QObject::connect(&s, &QAbstractSlider::valueChanged, [&](int) { sink += s.value(); });
    QElapsedTimer t; t.start();
    for (int i = 0; i < 100000; i++) s.setValue(i + 1);
    scenario("QSlider 100k setValue with a NOTIFY slot reading value()", QString("%1 ns/set (sink %2)").arg(t.nsecsElapsed() / 100000.0).arg(sink > 0));
  }
  return 0;
}
