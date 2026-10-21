// Qt observability probe (plans/research/REACTIVITY.md sections 3.2 and 4.5; milestone R0).
//
// Run by `tools/metadata/qt/probe.py --section observe`, which writes classes.inc (one
// `PROBE(QClass)` line per class of the metadata), builds this file against a Qt SDK, runs it and turns its output
// into metadata/probe/qt/observe/<version>.json. Seeded by docs/notes/scripts/reactivity/qt_observe_probe.cpp (the
// research sweep over 32 hand-picked classes); this version covers every default-constructible QObject class of the
// metadata, records the Q_PROPERTY flags from the metaobject, and diffs every readable property around each change,
// listening to every NOTIFY and other public signal, every QBindable notifier and the QEvents the object receives.
//
// stdout (TSV, one record per line):
//   #qt     <qVersion()>
//   #begin  <Class>                                   (the driver's crash bookkeeping)
//   T  <Class> object|abstract                        (abstract: not default-constructible here)
//   F  <owner> <prop> <readable> <writable> <constant> <bindable> <user> <notify signature or ->
//   S  <inst> <owner> <prop> <readback> <notify on change> <notify on same> <bindable on change> <events> <signals>
//   E  <inst> <cause> <owner> <prop> <notify> <changed> <bindable> <events> <signals>
//   C  <scenario> <note>
//   X  <inst> <cause> <events>                        (O4 section, milestone R3: the events of one cause)
//   W  <inst> <cause> <owner> <prop> <events>         (a QWidget-family property that changed during that cause)
//   P  <owner> <prop> <cause> <changed> <signals>     (name-paired signal candidates: a native change)
// `notify`: emissions of the property's own NOTIFY signal; `bindable`: QPropertyNotifier calls; `events`: the QEvent
// types the object received during the change; `signals`: the other public signals it emitted (O5 evidence).
#include <QtWidgets>
#include <QtTest/QSignalSpy>
#include <QtTest/QTest>
#include <cmath>
#include <cstdio>
#include <functional>
#include <map>
#include <memory>
#include <set>
#include <type_traits>
#include <vector>

static const QMetaEnum kEvents = QMetaEnum::fromType<QEvent::Type>();

static bool env_skip(const char *cls) {
  const QStringList l = QString::fromLocal8Bit(qgetenv("SVL_SKIP")).split(' ', Qt::SkipEmptyParts);
  return l.contains(QString::fromLatin1(cls));
}

struct Filter : QObject {
  QStringList seen;
  bool eventFilter(QObject *, QEvent *e) override {
    switch (e->type()) {
    case QEvent::Paint: case QEvent::UpdateRequest: case QEvent::UpdateLater: case QEvent::Timer:
    case QEvent::MetaCall: case QEvent::DeferredDelete: case QEvent::ChildPolished: case QEvent::Polish:
    case QEvent::PolishRequest: case QEvent::ChildAdded: case QEvent::ChildRemoved: case QEvent::LayoutRequest:
    case QEvent::DynamicPropertyChange:
      return false;
    default: break;
    }
    const char *k = kEvents.valueToKey(e->type());
    QString n = k ? QString::fromLatin1(k) : QString::number(e->type());
    if (!seen.contains(n)) seen << n;
    return false;
  }
};

static const QMetaObject *owner_of(const QMetaObject *mo, int i) {
  while (mo->superClass() && i < mo->propertyOffset()) mo = mo->superClass();
  return mo;
}

static bool skip_read(const char *n) {
  static const std::set<QByteArray> s = {"parent", "windowHandle", "screen", "nativeParentWidget", "palette", "font",
                                         "cursor", "locale", "styleSheet", "windowIcon", "icon", "pixmap", "movie",
                                         "picture", "document", "graphicsEffect"};
  return s.count(QByteArray(n));
}

// Everything a change of one object can show: values, NOTIFY and other signals, bindable notifiers, events.
struct Watch {
  QObject *o;
  const QMetaObject *mo;
  std::vector<std::unique_ptr<QSignalSpy>> spies;
  std::map<int, size_t> spy_of;  // signal method index -> spies[]
  std::map<int, int> bcount;     // property index -> notifier calls
  std::vector<QPropertyNotifier> notifiers;
  Filter f;
  std::vector<QVariant> snap;
  std::vector<bool> ok;

  explicit Watch(QObject *obj) : o(obj), mo(obj->metaObject()) {
    for (int i = 0; i < mo->methodCount(); i++) {
      QMetaMethod m = mo->method(i);
      if (m.methodType() != QMetaMethod::Signal || m.access() != QMetaMethod::Public) continue;
      auto spy = std::make_unique<QSignalSpy>(o, m);
      if (!spy->isValid()) continue;
      spy_of[i] = spies.size();
      spies.push_back(std::move(spy));
    }
    for (int i = 0; i < mo->propertyCount(); i++) {
      QMetaProperty p = mo->property(i);
      if (!p.isBindable()) continue;
      QUntypedBindable b = p.bindable(o);
      if (!b.isValid()) continue;
      notifiers.push_back(b.addNotifier([this, i] { bcount[i]++; }));
    }
    o->installEventFilter(&f);
  }
  ~Watch() { o->removeEventFilter(&f); }
  void reset() {
    for (auto &s : spies) s->clear();
    bcount.clear();
    f.seen.clear();
    snap.assign(mo->propertyCount(), QVariant());
    ok.assign(mo->propertyCount(), false);
    for (int i = 0; i < mo->propertyCount(); i++) {
      QMetaProperty p = mo->property(i);
      if (!p.isReadable() || skip_read(p.name()) || !p.metaType().isEqualityComparable()) continue;
      snap[i] = p.read(o);
      ok[i] = true;
    }
  }
  int notify_count(int i) const {
    QMetaProperty p = mo->property(i);
    if (!p.hasNotifySignal()) return -1;
    auto it = spy_of.find(p.notifySignalIndex());
    return it == spy_of.end() ? -1 : int(spies[it->second]->count());
  }
  // Public signals that fired, minus the NOTIFY signal of property `self` (none when self < 0): another property's
  // NOTIFY can be the signal that reports this one (QTextEdit::textChanged for plainText).
  QString signals_fired(int self) const {
    QStringList out;
    std::set<int> notifies;
    if (self >= 0 && mo->property(self).hasNotifySignal()) notifies.insert(mo->property(self).notifySignalIndex());
    for (auto &[idx, k] : spy_of)
      if (spies[k]->count() && !notifies.count(idx)) out << QString::fromLatin1(mo->method(idx).name());
    out.removeDuplicates();
    return out.join(',');
  }
  // E rows: every property other than `self` that changed, notified or had its notifier called.
  void effects(const char *inst, const QString &cause, int self) {
    const QString ev = f.seen.join(','), sig = signals_fired(-1);
    for (int i = 0; i < mo->propertyCount(); i++) {
      if (i == self) continue;
      QMetaProperty p = mo->property(i);
      bool changed = ok[i] && p.read(o) != snap[i];
      int n = notify_count(i), b = bcount.count(i) ? bcount.at(i) : 0;
      if (!changed && n <= 0 && b == 0) continue;
      printf("E\t%s\t%s\t%s\t%s\t%d\t%d\t%d\t%s\t%s\n", inst, qPrintable(cause), owner_of(mo, i)->className(), p.name(), n,
             changed ? 1 : 0, p.isBindable() ? b : -1, qPrintable(ev), qPrintable(sig));
    }
  }
};

static bool next_value(const QMetaProperty &p, const QVariant &cur, QVariant &out) {
  if (p.isEnumType()) {
    if (p.isFlagType()) return false;
    QMetaEnum me = p.enumerator();
    int c = cur.toInt();
    for (int i = 0; i < me.keyCount(); i++)
      if (me.value(i) != c) { out = QVariant(me.value(i)); return true; }
    return false;
  }
  switch (p.metaType().id()) {
  case QMetaType::Bool: out = !cur.toBool(); return true;
  case QMetaType::Int: out = cur.toInt() + 1; return true;
  case QMetaType::UInt: out = cur.toUInt() + 1; return true;
  case QMetaType::LongLong: out = cur.toLongLong() + 1; return true;
  case QMetaType::Double:
  case QMetaType::Float: {
    // c + 1 vanishes next to an infinite or huge default (QDoubleValidator::bottom is -inf): 0 and 1 follow.
    const double c = cur.toDouble();
    for (double v : {c + 1.0, 0.0, 1.0})
      if (std::isfinite(v) && v != c) {
        out = p.metaType().id() == QMetaType::Float ? QVariant(float(v)) : QVariant(v);
        return true;
      }
    return false;
  }
  case QMetaType::QString: out = cur.toString() + "x"; return true;
  default: return false;
  }
}

static std::set<QByteArray> swept;  // "<owner>:<prop>" of abstract owners already swept on a subclass

static void flags_of(const QMetaObject *mo) {
  for (int i = mo->propertyOffset(); i < mo->propertyCount(); i++) {
    QMetaProperty p = mo->property(i);
    printf("F\t%s\t%s\t%d\t%d\t%d\t%d\t%d\t%s\n", mo->className(), p.name(), p.isReadable(), p.isWritable(), p.isConstant(),
           p.isBindable(), p.isUser(), p.hasNotifySignal() ? p.notifySignal().methodSignature().constData() : "-");
  }
}

static bool is_concrete(const QMetaObject *mo, const std::set<QByteArray> &concrete) {
  return concrete.count(QByteArray(mo->className()));
}

static std::set<QByteArray> concrete_classes;

// Every property is set on a fresh instance, as the generated verification test does: a change that depends on an
// earlier set (QGroupBox `checked` after `checkable`) shows that dependency as a side effect only.
static void sweep(const std::function<QObject *()> &make, const char *inst) {
  QObject *proto = make();
  const QMetaObject *mo = proto->metaObject();
  delete proto;
  QCoreApplication::processEvents();
  for (int i = 0; i < mo->propertyCount(); i++) {
    QMetaProperty p = mo->property(i);
    const QMetaObject *own = owner_of(mo, i);
    QByteArray key = QByteArray(own->className()) + ":" + p.name();
    bool mine = own == mo;
    bool shared = !is_concrete(own, concrete_classes) && !swept.count(key);
    if (!(mine || shared) || !p.isWritable() || !p.isReadable() || p.isConstant()) continue;
    if (!qstrcmp(p.name(), "visible") || !qstrcmp(p.name(), "objectName") || !qstrcmp(p.name(), "windowState") ||
        !qstrcmp(p.name(), "windowModality") || !qstrcmp(p.name(), "nativeMenuBar"))
      continue;
    std::unique_ptr<QObject> holder(make());
    QObject *o = holder.get();
    Watch w(o);
    QVariant cur = p.read(o), nv;
    if (!next_value(p, cur, nv) || nv == cur) continue;
    if (!mine) swept.insert(key);
    w.reset();
    p.write(o, nv);
    QCoreApplication::processEvents();
    QVariant after = p.read(o);
    int nchange = w.notify_count(i), bchange = w.bcount.count(i) ? w.bcount[i] : 0;
    QString ev = w.f.seen.join(','), sig = w.signals_fired(i);
    w.effects(inst, QString("set:%1:%2").arg(own->className(), p.name()), i);
    const char *rb = after == nv ? "exact" : after == cur ? "ignored" : "normalized";
    w.reset();
    p.write(o, after);
    QCoreApplication::processEvents();
    int nsame = w.notify_count(i);
    printf("S\t%s\t%s\t%s\t%s\t%d\t%d\t%d\t%s\t%s\n", inst, own->className(), p.name(), rb, nchange, nsame,
           p.isBindable() ? bchange : -1, qPrintable(ev), qPrintable(sig));
  }
}

template <class T> static void mark_concrete(const char *name) {
  if constexpr (std::is_base_of_v<QObject, T> && std::is_default_constructible_v<T> && !std::is_abstract_v<T>)
    concrete_classes.insert(QByteArray(name));
}

template <class T> static void probe_class(const char *name) {
  if constexpr (std::is_base_of_v<QObject, T>) {
    flags_of(&T::staticMetaObject);
    if constexpr (std::is_default_constructible_v<T> && !std::is_abstract_v<T>) {
      printf("T\t%s\tobject\n", name);
      if (env_skip(name)) return;
      printf("#begin\t%s\n", name);
      sweep([] {
        T *o = new T;
        if constexpr (std::is_base_of_v<QWidget, T>) {
          static_cast<QWidget *>(o)->resize(200, 100);
          static_cast<QWidget *>(o)->show();
        }
        QCoreApplication::processEvents();
        return static_cast<QObject *>(o);
      }, name);
      QCoreApplication::processEvents();
    } else {
      printf("T\t%s\tabstract\n", name);
    }
  }
}

// ---- scenarios: changes that are not property sets ----
struct Scenario {
  Watch w;
  const char *inst, *name;
  Scenario(QObject *o, const char *inst, const char *name) : w(o), inst(inst), name(name) { w.reset(); }
  ~Scenario() {
    QCoreApplication::processEvents();
    w.effects(inst, QString("scenario:%1").arg(name), -1);
  }
};

static void scenarios() {
  {
    QLineEdit e;
    e.show();
    e.setFocus();
    QCoreApplication::processEvents();
    { Scenario s(&e, "QLineEdit", "QLineEdit keyClicks"); QTest::keyClicks(&e, "xyz"); }
    { Scenario s(&e, "QLineEdit", "QLineEdit setText"); e.setText("ab"); }
  }
  {
    QPlainTextEdit e;
    e.show();
    e.setFocus();
    QCoreApplication::processEvents();
    { Scenario s(&e, "QPlainTextEdit", "QPlainTextEdit keyClicks"); QTest::keyClicks(&e, "xyz"); }
    { Scenario s(e.document(), "QTextDocument", "QPlainTextEdit keyClicks: its document"); QTest::keyClicks(&e, "abc"); }
  }
  {
    QTextEdit e;
    e.show();
    e.setFocus();
    QCoreApplication::processEvents();
    { Scenario s(&e, "QTextEdit", "QTextEdit keyClicks"); QTest::keyClicks(&e, "xyz"); }
  }
  {
    QSpinBox b;
    b.setRange(0, 99);
    b.show();
    b.setFocus();
    QCoreApplication::processEvents();
    { Scenario s(&b, "QSpinBox", "QSpinBox keyClicks"); QTest::keyClick(&b, Qt::Key_Up); }
  }
  {
    QSlider sl;
    sl.setRange(0, 100);
    sl.setValue(80);
    { Scenario s(&sl, "QSlider", "QSlider setMaximum below value"); sl.setMaximum(50); }
  }
  {
    QCheckBox c("x");
    c.show();
    QCoreApplication::processEvents();
    { Scenario s(&c, "QCheckBox", "QCheckBox mouseClick"); QTest::mouseClick(&c, Qt::LeftButton); }
  }
  {
    QWidget parent;
    QWidget *child = new QWidget(&parent);
    parent.show();
    QCoreApplication::processEvents();
    { Scenario s(child, "QWidget", "parent setEnabled(false)"); parent.setEnabled(false); }
    { Scenario s(child, "QWidget", "parent hide()"); parent.hide(); }
  }
  {
    QWidget w;
    { Scenario s(&w, "QWidget", "hidden QWidget resize"); w.resize(300, 200); }
    printf("C\thidden QWidget resize\tResize is delivered at show(), not at the resize\n");
  }
  {
    QTabWidget t;
    for (int i = 0; i < 3; i++) t.addTab(new QWidget, QString::number(i));
    t.setCurrentIndex(2);
    { Scenario s(&t, "QTabWidget", "QTabWidget remove the current tab"); t.removeTab(2); }
  }
  {
    QTimer t;
    { Scenario s(&t, "QTimer", "QTimer start"); t.start(1000); }
    { Scenario s(&t, "QTimer", "QTimer stop"); t.stop(); }
  }
  {
    QComboBox c;
    { Scenario s(&c, "QComboBox", "QComboBox addItems"); c.addItems({"a", "b"}); }
  }
}

// ---- O4 (milestone R3): which QEvent announces which QWidget property ----
//
// Every writable QWidget property (any value type, not just the scalars of the sweep above) is set on a fresh shown
// widget and then set back; scenarios cover the read-only ones (window states, focus, activation, parent changes).
// For each cause, the events the widget received (X) and every property of the widget that changed (W):
//   X  <inst> <cause> <events>
//   W  <inst> <cause> <owner> <prop> <events>
// probe.py derives the property -> QEvent::Type table from these rows (rule O4).

static bool o4_skip(const char *n) {
  static const std::set<QByteArray> s = {"windowHandle", "screen", "objectName", "windowFlags", "modal"};
  return s.count(QByteArray(n));
}

static bool next_any(const QMetaProperty &p, const QVariant &cur, QVariant &out) {
  if (next_value(p, cur, out)) return true;
  const auto bump = [](int v) { return v > 1000 ? v - 11 : v + 11; };
  switch (p.metaType().id()) {
  case QMetaType::QRect: {
    const QRect r = cur.toRect();
    out = QRect(r.x() + 7, r.y() + 5, bump(r.width()), bump(r.height()));
    return true;
  }
  case QMetaType::QSize: {
    const QSize s = cur.toSize();
    out = QSize(bump(s.width()), bump(s.height()));
    return true;
  }
  case QMetaType::QPoint: out = cur.toPoint() + QPoint(7, 5); return true;
  case QMetaType::QFont: {
    QFont f = cur.value<QFont>();
    f.setPointSize(f.pointSize() > 0 ? f.pointSize() + 3 : 15);
    out = f;
    return true;
  }
  case QMetaType::QPalette: {
    QPalette pl = cur.value<QPalette>();
    pl.setColor(QPalette::Window, pl.color(QPalette::Window) == QColor(1, 2, 3) ? QColor(4, 5, 6) : QColor(1, 2, 3));
    out = pl;
    return true;
  }
  case QMetaType::QCursor:
    out = QCursor(cur.value<QCursor>().shape() == Qt::CrossCursor ? Qt::ArrowCursor : Qt::CrossCursor);
    return true;
  case QMetaType::QLocale:
    out = QLocale(cur.toLocale().language() == QLocale::German ? QLocale::French : QLocale::German);
    return true;
  case QMetaType::QSizePolicy: {
    QSizePolicy sp = cur.value<QSizePolicy>();
    sp.setHorizontalPolicy(sp.horizontalPolicy() == QSizePolicy::Fixed ? QSizePolicy::Expanding : QSizePolicy::Fixed);
    out = QVariant::fromValue(sp);
    return true;
  }
  case QMetaType::QIcon: {
    QPixmap pm(8, 8);
    pm.fill(Qt::red);
    out = QIcon(pm);
    return true;
  }
  default: break;
  }
  if (p.isFlagType()) {  // toggle the first non-zero flag
    const QMetaEnum me = p.enumerator();
    for (int i = 0; i < me.keyCount(); i++)
      if (me.value(i) != 0) {
        QVariant v(cur.toInt() ^ me.value(i));
        if (!v.convert(cur.metaType())) return false;
        out = v;
        return true;
      }
  }
  return false;
}

// The properties of one widget, and the events it receives, around one cause.
struct O4Watch {
  QWidget *w;
  const QMetaObject *mo;
  Filter f;
  std::vector<QVariant> snap;
  std::vector<bool> ok;
  explicit O4Watch(QWidget *x) : w(x), mo(x->metaObject()) { w->installEventFilter(&f); reset(); }
  ~O4Watch() { w->removeEventFilter(&f); }
  void reset() {
    f.seen.clear();
    snap.assign(mo->propertyCount(), QVariant());
    ok.assign(mo->propertyCount(), false);
    for (int i = 0; i < mo->propertyCount(); i++) {
      QMetaProperty p = mo->property(i);
      if (!p.isReadable() || o4_skip(p.name()) || !p.metaType().isEqualityComparable()) continue;
      snap[i] = p.read(w);
      ok[i] = true;
    }
  }
  void report(const char *inst, const QString &cause) {
    QCoreApplication::processEvents();
    const QString ev = f.seen.join(',');
    printf("X\t%s\t%s\t%s\n", inst, qPrintable(cause), qPrintable(ev));
    for (int i = 0; i < mo->propertyCount(); i++) {
      QMetaProperty p = mo->property(i);
      if (!ok[i] || p.read(w) == snap[i]) continue;
      printf("W\t%s\t%s\t%s\t%s\t%s\n", inst, qPrintable(cause), owner_of(mo, i)->className(), p.name(), qPrintable(ev));
    }
    reset();
  }
};

static void o4_sweep(const char *inst, const std::function<QWidget *()> &make) {
  QWidget *proto = make();
  const QMetaObject *mo = proto->metaObject();
  delete proto;
  for (int i = 0; i < mo->propertyCount(); i++) {
    QMetaProperty p = mo->property(i);
    const QMetaObject *own = owner_of(mo, i);
    // QWidget's own properties on every instance; a subclass's own ones (readOnly) on that subclass.
    if (own != &QWidget::staticMetaObject && own != mo) continue;
    if (!p.isWritable() || !p.isReadable() || p.isConstant() || o4_skip(p.name())) continue;
    std::unique_ptr<QWidget> w(make());
    QVariant cur = p.read(w.get()), nv;
    if (!next_any(p, cur, nv) || nv == cur) continue;
    O4Watch watch(w.get());
    const QString cause = QString("set:%1:%2").arg(own->className(), p.name());
    p.write(w.get(), nv);
    watch.report(inst, cause);
    p.write(w.get(), cur);
    watch.report(inst, cause + ":back");
  }
}

static QWidget *shown(QWidget *w) {
  w->resize(200, 100);
  w->show();
  QCoreApplication::processEvents();
  return w;
}

static void o4() {
  o4_sweep("QWidget", [] { return shown(new QWidget); });
  // A child widget in a shown window: hiding it is not a window deactivation, moving it no window move.
  o4_sweep("QWidget(child)", [] {
    auto *parent = new QWidget;
    auto *child = new QWidget(parent);
    child->setGeometry(10, 10, 80, 40);
    QObject::connect(child, &QObject::destroyed, parent, [parent] { parent->deleteLater(); });
    shown(parent);
    return child;
  });
  o4_sweep("QLineEdit", [] { return shown(new QLineEdit); });
  o4_sweep("QTextEdit", [] { return shown(new QTextEdit); });
  o4_sweep("QPlainTextEdit", [] { return shown(new QPlainTextEdit); });
  o4_sweep("QSpinBox", [] { return shown(new QSpinBox); });
  // Read-only properties: window states, focus, activation; and what a parent's change does to a child.
  {
    std::unique_ptr<QWidget> w(shown(new QWidget));
    O4Watch watch(w.get());
    w->showMinimized(); watch.report("QWidget", "scenario:showMinimized");
    w->showNormal(); watch.report("QWidget", "scenario:showNormal after minimized");
    w->showMaximized(); watch.report("QWidget", "scenario:showMaximized");
    w->showNormal(); watch.report("QWidget", "scenario:showNormal after maximized");
    w->showFullScreen(); watch.report("QWidget", "scenario:showFullScreen");
    w->showNormal(); watch.report("QWidget", "scenario:showNormal after fullScreen");
    w->setWindowState(Qt::WindowMaximized); watch.report("QWidget", "scenario:setWindowState(maximized)");
    w->setWindowState(Qt::WindowNoState); watch.report("QWidget", "scenario:setWindowState(none)");
    w->move(40, 30); watch.report("QWidget", "scenario:move");
    w->resize(230, 120); watch.report("QWidget", "scenario:resize");
    w->hide(); watch.report("QWidget", "scenario:hide");
    w->resize(260, 140); watch.report("QWidget", "scenario:hidden resize");
    w->move(50, 60); watch.report("QWidget", "scenario:hidden move");
    w->show(); watch.report("QWidget", "scenario:show after hidden resize");
  }
  {
    std::unique_ptr<QWidget> a(shown(new QWidget)), b(shown(new QWidget));
    O4Watch wa(a.get());
    b->activateWindow(); QApplication::setActiveWindow(b.get()); wa.report("QWidget", "scenario:another window activated");
    a->activateWindow(); QApplication::setActiveWindow(a.get()); wa.report("QWidget", "scenario:activateWindow");
  }
  {
    std::unique_ptr<QWidget> parent(new QWidget);
    auto *child = new QLineEdit(parent.get());
    auto *other = new QLineEdit(parent.get());
    other->move(0, 40);
    shown(parent.get());
    parent->activateWindow();
    QApplication::setActiveWindow(parent.get());
    QCoreApplication::processEvents();
    O4Watch watch(child);
    child->setFocus(); watch.report("QLineEdit", "scenario:setFocus");
    other->setFocus(); watch.report("QLineEdit", "scenario:focus moved away");
    parent->setEnabled(false); watch.report("QLineEdit", "scenario:parent setEnabled(false)");
    parent->setEnabled(true); watch.report("QLineEdit", "scenario:parent setEnabled(true)");
    QFont f = parent->font();
    f.setPointSize(f.pointSize() + 4);
    parent->setFont(f); watch.report("QLineEdit", "scenario:parent setFont");
    QPalette pl = parent->palette();
    pl.setColor(QPalette::Window, QColor(9, 8, 7));
    parent->setPalette(pl); watch.report("QLineEdit", "scenario:parent setPalette");
    parent->setLocale(QLocale(QLocale::Japanese)); watch.report("QLineEdit", "scenario:parent setLocale");
    parent->setLayoutDirection(Qt::RightToLeft); watch.report("QLineEdit", "scenario:parent setLayoutDirection");
    parent->setStyleSheet("QLineEdit { color: red; }"); watch.report("QLineEdit", "scenario:parent setStyleSheet");
    parent->resize(400, 300); watch.report("QLineEdit", "scenario:parent resize");
    parent->hide(); watch.report("QLineEdit", "scenario:parent hide()");
    parent->show(); watch.report("QLineEdit", "scenario:parent show()");
    parent->setWindowModified(true); watch.report("QLineEdit", "scenario:parent setWindowModified(true)");
    // Its window loses activation and gets it back: WindowDeactivate/WindowActivate without a visibility change.
    std::unique_ptr<QWidget> other_win(shown(new QWidget));
    other_win->activateWindow(); QApplication::setActiveWindow(other_win.get()); watch.report("QLineEdit", "scenario:another window activated");
    parent->activateWindow(); QApplication::setActiveWindow(parent.get()); watch.report("QLineEdit", "scenario:its window activated");
    // Hidden with an inactive window: no WindowDeactivate comes along.
    other_win->activateWindow(); QApplication::setActiveWindow(other_win.get()); watch.report("QLineEdit", "scenario:another window activated again");
    parent->hide(); watch.report("QLineEdit", "scenario:inactive parent hide()");
    parent->show(); watch.report("QLineEdit", "scenario:inactive parent show()");
  }
}

// ---- name-paired signal candidates (milestone R3): native changes of a property without NOTIFY ----
//   P  <owner> <prop> <cause> <changed 0|1> <signals the object emitted>
template <class T, class G>
static void pair_case(T *o, const char *owner, const char *prop, const char *cause, G get, const std::function<void()> &act) {
  Watch w(o);
  w.reset();
  const QVariant before = QVariant::fromValue(get());
  act();
  QCoreApplication::processEvents();
  const bool changed = QVariant::fromValue(get()) != before;
  printf("P\t%s\t%s\t%s\t%d\t%s\n", owner, prop, cause, changed ? 1 : 0, qPrintable(w.signals_fired(-1)));
}

static void pairs() {
  {
    QComboBox c;
    c.addItem("a", 1);
    c.addItem("b", 2);
    c.addItem("c", 3);
    auto get = [&] { return c.currentData(); };
    pair_case(&c, "QComboBox", "currentData", "setCurrentIndex", get, [&] { c.setCurrentIndex(1); });
    pair_case(&c, "QComboBox", "currentData", "setItemData(current)", get, [&] { c.setItemData(c.currentIndex(), 7); });
    pair_case(&c, "QComboBox", "currentData", "removeItem(current)", get, [&] { c.removeItem(c.currentIndex()); });
    pair_case(&c, "QComboBox", "currentData", "setCurrentText", get, [&] { c.setCurrentText("c"); });
  }
  {
    QGraphicsScene s;
    auto get = [&] { return s.sceneRect(); };
    pair_case(&s, "QGraphicsScene", "sceneRect", "addRect (grows the unset rect)", get, [&] { s.addRect(0, 0, 50, 40); });
    pair_case(&s, "QGraphicsScene", "sceneRect", "setSceneRect", get, [&] { s.setSceneRect(0, 0, 300, 200); });
    pair_case(&s, "QGraphicsScene", "sceneRect", "addRect (fixed rect)", get, [&] { s.addRect(500, 500, 50, 40); });
  }
  for (int mode = 0; mode < 3; mode++) {
    QInputDialog d;
    d.setInputMode(mode == 0 ? QInputDialog::IntInput : mode == 1 ? QInputDialog::DoubleInput : QInputDialog::TextInput);
    d.show();
    QCoreApplication::processEvents();
    if (mode == 0) {
      auto get = [&] { return d.intValue(); };
      pair_case(&d, "QInputDialog", "intValue", "setIntValue", get, [&] { d.setIntValue(5); });
      pair_case(&d, "QInputDialog", "intValue", "setIntMaximum below value", get, [&] { d.setIntMaximum(3); });
      pair_case(&d, "QInputDialog", "intValue", "keyClicks", get, [&] {
        if (auto *sb = d.findChild<QSpinBox *>()) { sb->setFocus(); QTest::keyClick(sb, Qt::Key_Down); }
      });
    } else if (mode == 1) {
      auto get = [&] { return d.doubleValue(); };
      pair_case(&d, "QInputDialog", "doubleValue", "setDoubleValue", get, [&] { d.setDoubleValue(2.5); });
      pair_case(&d, "QInputDialog", "doubleValue", "setDoubleMaximum below value", get, [&] { d.setDoubleMaximum(1.0); });
      pair_case(&d, "QInputDialog", "doubleValue", "keyClicks", get, [&] {
        if (auto *sb = d.findChild<QDoubleSpinBox *>()) { sb->setFocus(); QTest::keyClick(sb, Qt::Key_Down); }
      });
    } else {
      auto get = [&] { return d.textValue(); };
      pair_case(&d, "QInputDialog", "textValue", "setTextValue", get, [&] { d.setTextValue("abc"); });
      pair_case(&d, "QInputDialog", "textValue", "keyClicks", get, [&] {
        if (auto *le = d.findChild<QLineEdit *>()) { le->setFocus(); QTest::keyClicks(le, "xy"); }
      });
    }
  }
  {
    QLineEdit e;
    e.show();
    e.setFocus();
    QCoreApplication::processEvents();
    e.setText("hello world");
    auto sel = [&] { return e.selectedText(); };
    auto has = [&] { return e.hasSelectedText(); };
    auto redo = [&] { return e.isRedoAvailable(); };
    pair_case(&e, "QLineEdit", "selectedText", "setSelection", sel, [&] { e.setSelection(0, 5); });
    pair_case(&e, "QLineEdit", "selectedText", "selectAll", sel, [&] { e.selectAll(); });
    pair_case(&e, "QLineEdit", "selectedText", "deselect", sel, [&] { e.deselect(); });
    pair_case(&e, "QLineEdit", "hasSelectedText", "setSelection", has, [&] { e.setSelection(1, 3); });
    pair_case(&e, "QLineEdit", "hasSelectedText", "setText clears the selection", has, [&] { e.setText("other"); });
    pair_case(&e, "QLineEdit", "selectedText", "shift+left (user)", sel, [&] { e.end(false); QTest::keyClick(&e, Qt::Key_Left, Qt::ShiftModifier); });
    pair_case(&e, "QLineEdit", "hasSelectedText", "typing over the selection (user)", has, [&] { QTest::keyClicks(&e, "z"); });
    pair_case(&e, "QLineEdit", "redoAvailable", "undo", redo, [&] { e.undo(); });
    pair_case(&e, "QLineEdit", "redoAvailable", "redo", redo, [&] { e.redo(); });
    pair_case(&e, "QLineEdit", "redoAvailable", "undo (user ctrl+z)", redo, [&] { QTest::keyClick(&e, Qt::Key_Z, Qt::ControlModifier); });
    pair_case(&e, "QLineEdit", "redoAvailable", "typing (user)", redo, [&] { QTest::keyClicks(&e, "q"); });
  }
  {
    QMainWindow w;
    auto get = [&] { return w.iconSize(); };
    pair_case(&w, "QMainWindow", "iconSize", "setIconSize", get, [&] { w.setIconSize(QSize(41, 41)); });
    pair_case(&w, "QMainWindow", "iconSize", "setIconSize(invalid): back to the style's", get, [&] { w.setIconSize(QSize()); });
  }
  {
    QPlainTextEdit e;
    e.show();
    e.setFocus();
    QCoreApplication::processEvents();
    auto get = [&] { return e.blockCount(); };
    pair_case(&e, "QPlainTextEdit", "blockCount", "appendPlainText", get, [&] { e.appendPlainText("x"); });
    pair_case(&e, "QPlainTextEdit", "blockCount", "setPlainText", get, [&] { e.setPlainText("a\nb\nc\nd"); });
    pair_case(&e, "QPlainTextEdit", "blockCount", "Return (user)", get, [&] { QTest::keyClick(&e, Qt::Key_Return); });
    pair_case(&e, "QPlainTextEdit", "blockCount", "clear", get, [&] { e.clear(); });
    QTextDocument *doc = e.document();
    auto dget = [&] { return doc->blockCount(); };
    pair_case(doc, "QTextDocument", "blockCount", "setPlainText", dget, [&] { doc->setPlainText("1\n2\n3"); });
    pair_case(doc, "QTextDocument", "blockCount", "Return in its editor (user)", dget, [&] { QTest::keyClick(&e, Qt::Key_Return); });
    pair_case(doc, "QTextDocument", "blockCount", "undo", dget, [&] { doc->undo(); });
  }
  {
    QTemporaryDir dir;
    const QString a = dir.filePath("a.html"), b = dir.filePath("b.html");
    for (const QString &f : {a, b}) {
      QFile file(f);
      if (file.open(QIODevice::WriteOnly)) file.write("<p>x</p>");
    }
    QTextBrowser t;
    auto get = [&] { return t.source(); };
    pair_case(&t, "QTextBrowser", "source", "setSource", get, [&] { t.setSource(QUrl::fromLocalFile(a)); });
    pair_case(&t, "QTextBrowser", "source", "setSource (another)", get, [&] { t.setSource(QUrl::fromLocalFile(b)); });
    pair_case(&t, "QTextBrowser", "source", "backward", get, [&] { t.backward(); });
    pair_case(&t, "QTextBrowser", "source", "clear", get, [&] { t.clear(); });
  }
}

int main(int argc, char **argv) {
  setvbuf(stdout, nullptr, _IOLBF, 0);
  QApplication app(argc, argv);
  printf("#qt\t%s\n", qVersion());
  // First the classes that get an instance (their own props are swept on it), then the probe in file order.
#define PROBE(T) mark_concrete<T>(#T);
#include "classes.inc"
#undef PROBE
#define PROBE(T) probe_class<T>(#T);
#include "classes.inc"
  if (!env_skip("@scenarios")) {
    printf("#begin\t@scenarios\n");
    scenarios();
  }
  if (!env_skip("@o4")) {
    printf("#begin\t@o4\n");
    o4();
  }
  if (!env_skip("@pairs")) {
    printf("#begin\t@pairs\n");
    pairs();
  }
  printf("#end\n");
  return 0;
}
