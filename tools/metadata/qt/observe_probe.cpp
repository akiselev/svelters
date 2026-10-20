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
// `notify`: emissions of the property's own NOTIFY signal; `bindable`: QPropertyNotifier calls; `events`: the QEvent
// types the object received during the change; `signals`: the other public signals it emitted (O5 evidence).
#include <QtWidgets>
#include <QtTest/QSignalSpy>
#include <QtTest/QTest>
#include <cstdio>
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
  // Public signals that fired, minus the NOTIFY signal of `self` (or of every property when self < 0).
  QString signals_fired(int self) const {
    QStringList out;
    std::set<int> notifies;
    for (int i = 0; i < mo->propertyCount(); i++)
      if (mo->property(i).hasNotifySignal() && (self < 0 || i == self)) notifies.insert(mo->property(i).notifySignalIndex());
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
  case QMetaType::Double: out = cur.toDouble() + 1.0; return true;
  case QMetaType::Float: out = cur.toFloat() + 1.0f; return true;
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

static void sweep(QObject *o, const char *inst) {
  const QMetaObject *mo = o->metaObject();
  Watch w(o);
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
    QVariant cur = p.read(o), nv;
    if (!next_value(p, cur, nv)) continue;
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

template <class T> static void probe_class(const char *name) {
  if constexpr (std::is_base_of_v<QObject, T>) {
    flags_of(&T::staticMetaObject);
    if constexpr (std::is_default_constructible_v<T> && !std::is_abstract_v<T>) {
      printf("T\t%s\tobject\n", name);
      if (env_skip(name)) return;
      printf("#begin\t%s\n", name);
      T *o = new T;
      if constexpr (std::is_base_of_v<QWidget, T>) {
        static_cast<QWidget *>(o)->resize(200, 100);
        static_cast<QWidget *>(o)->show();
      }
      QCoreApplication::processEvents();
      sweep(o, name);
      delete o;
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

int main(int argc, char **argv) {
  setvbuf(stdout, nullptr, _IOLBF, 0);
  QApplication app(argc, argv);
  printf("#qt\t%s\n", qVersion());
#define PROBE(T) concrete_classes.insert(#T);
#define CONCRETE_ONLY
#include "classes.inc"
#undef PROBE
#undef CONCRETE_ONLY
#define PROBE(T) probe_class<T>(#T);
#include "classes.inc"
  if (!env_skip("@scenarios")) {
    printf("#begin\t@scenarios\n");
    scenarios();
  }
  printf("#end\n");
  return 0;
}
