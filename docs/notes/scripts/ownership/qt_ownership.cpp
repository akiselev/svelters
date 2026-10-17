// Ownership experiments for svelters (Qt 6). Each case prints "CASE <name>: <facts>".
// Mini handle table mirrors tools/metadata/qt/templates/backend.cpp.in:
// QPointer + QObject::destroyed -> on_destroyed.
#include <QtWidgets>
#include <QtTest/QTest>
#include <cstdio>
#include <map>
#include <vector>
#include <string>

static int g_case_fail = 0;
#define CHECK(c) do { bool _ok = (c); std::printf("  %s %s\n", _ok ? "ok  " : "FAIL", #c); if (!_ok) ++g_case_fail; } while (0)

struct Table {
  struct E { QPointer<QObject> o; QMetaObject::Connection c; };
  std::map<uint64_t, E> t;
  uint64_t next = 1;
  std::vector<std::string> log;  // on_destroyed log
  uint64_t add(QObject *o) {
    uint64_t h = next++;
    auto c = QObject::connect(o, &QObject::destroyed, [this, h](QObject *obj) {
      auto it = t.find(h);
      // What does the world look like while `destroyed` runs?
      bool qp_null = it != t.end() && it->second.o.isNull();
      bool is_widget = qobject_cast<QWidget *>(obj) != nullptr;  // QWidget part already destroyed?
      log.push_back("destroyed #" + std::to_string(h) + (qp_null ? " qpointer=null" : " qpointer=set") +
                    (is_widget ? " cast=QWidget" : " cast=QObject"));
      if (it != t.end()) t.erase(it);
    });
    t[h] = {o, c};
    return h;
  }
  QObject *get(uint64_t h) { auto it = t.find(h); return it == t.end() ? nullptr : it->second.o.data(); }
  bool live(uint64_t h) { return get(h) != nullptr; }
  void destroy(uint64_t h) {
    auto it = t.find(h);
    if (it == t.end()) return;
    QObject *o = it->second.o.data();
    QObject::disconnect(it->second.c);
    t.erase(it);
    if (o) o->deleteLater();
  }
};

static void flush() {
  QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
  QCoreApplication::processEvents();
}

static void dumplog(Table &T) {
  for (auto &s : T.log) std::printf("    log: %s\n", s.c_str());
  T.log.clear();
}

// 1. remove from layout, keep detached, re-insert (same parent and another parent)
static void case_detach_reinsert() {
  std::printf("CASE detach_reinsert\n");
  Table T;
  auto *win = new QWidget; auto *lay = new QVBoxLayout(win);
  auto *other = new QWidget; auto *lay2 = new QVBoxLayout(other);
  auto *a = new QPushButton("a"); auto *b = new QPushButton("b");
  uint64_t hw = T.add(win), ho = T.add(other), ha = T.add(a), hb = T.add(b);
  lay->addWidget(a); lay->addWidget(b); win->show(); flush();
  // remove: removeWidget leaves it parented; backend's `then` step reparents to null
  lay->removeWidget(a);
  CHECK(a->parent() == win);
  a->setParent(nullptr);
  CHECK(a->parent() == nullptr && a->isWindow() && !a->isVisible());
  // parent window destroyed while `a` is detached: a survives (no parent), handle still live
  delete win; flush();
  CHECK(T.live(ha));
  CHECK(!T.live(hb));    // b was a child, deleted with win
  CHECK(!T.live(hw));
  dumplog(T);
  // re-insert into another parent: works, no flash as a top-level (was never shown as window)
  lay2->addWidget(a); other->show(); flush();
  CHECK(a->parent() == other && a->isVisible());
  // re-inserting a widget that is still in another layout: Qt silently reparents, old layout keeps a dangling QWidgetItem?
  auto *third = new QWidget; auto *lay3 = new QVBoxLayout(third);
  lay3->addWidget(a); flush();
  CHECK(a->parent() == third);
  CHECK(lay2->indexOf(a) == -1);   // Qt removes it from the old layout on reparent (childEvent)
  T.destroy(ho); T.destroy(ha); delete third; flush();
  dumplog(T);
}

// 2. native close of a window while JS holds handles
static void case_native_close() {
  std::printf("CASE native_close\n");
  Table T;
  // default: close() hides only
  auto *w1 = new QWidget; auto *l1 = new QVBoxLayout(w1); auto *c1 = new QLineEdit; l1->addWidget(c1);
  uint64_t h1 = T.add(w1), hc1 = T.add(c1);
  w1->show(); flush(); w1->close(); flush();
  CHECK(T.live(h1) && T.live(hc1) && !w1->isVisible());
  // WA_DeleteOnClose: children die first, then window; QPointer state inside destroyed recorded
  auto *w2 = new QWidget; auto *l2 = new QVBoxLayout(w2); auto *c2 = new QLineEdit; l2->addWidget(c2);
  w2->setAttribute(Qt::WA_DeleteOnClose);
  uint64_t h2 = T.add(w2), hc2 = T.add(c2);
  w2->show(); flush(); w2->close();
  CHECK(T.live(h2));             // deletion is deferred (deleteLater)
  flush();
  CHECK(!T.live(h2) && !T.live(hc2));
  dumplog(T);
  // a detached (removed) child of the closed window survives
  T.destroy(h1); T.destroy(hc1); flush();
  // quitOnLastWindowClosed: is it on by default?
  // quitOnLastWindowClosed default printed in main
}

// 3. owner deletes a borrowed child
static void case_borrowed() {
  std::printf("CASE borrowed\n");
  Table T;
  auto *combo = new QComboBox; combo->setEditable(true);
  uint64_t hcombo = T.add(combo);
  uint64_t hle = T.add(combo->lineEdit());   // borrowed: we did not create it
  combo->setEditable(false); flush();       // owner deletes it
  CHECK(!T.live(hle));
  dumplog(T);
  combo->setEditable(true);
  CHECK(combo->lineEdit() != nullptr);       // a NEW line edit; old handle must not alias it
  auto *sa = new QScrollArea; uint64_t hsa = T.add(sa);
  uint64_t hvp = T.add(sa->viewport());
  sa->setViewport(new QWidget); flush();
  CHECK(!T.live(hvp));
  // QFormLayout::removeRow deletes both widgets
  auto *f = new QWidget; auto *fl = new QFormLayout(f);
  auto *lab = new QLabel("x"); auto *fld = new QLineEdit;
  fl->addRow(lab, fld); uint64_t hl = T.add(lab), hf = T.add(fld);
  fl->removeRow(0); flush();
  CHECK(!T.live(hl) && !T.live(hf));
  // QMainWindow: setCentralWidget deletes old, takeCentralWidget unparents
  auto *mw = new QMainWindow; auto *c1 = new QLabel("1"); auto *c2 = new QLabel("2");
  uint64_t hc1 = T.add(c1); mw->setCentralWidget(c1); mw->setCentralWidget(c2); flush();
  CHECK(!T.live(hc1));
  QWidget *taken = mw->takeCentralWidget();
  CHECK(taken == c2 && c2->parent() == nullptr);
  delete c2;
  dumplog(T);
  T.destroy(hcombo); T.destroy(hsa); delete f; delete mw; flush();
}

// 4. deleting during signal emission
static void case_delete_in_signal() {
  std::printf("CASE delete_in_signal\n");
  Table T;
  // (a) deleteLater of the sender inside its own clicked(): later slots still run, object alive until loop
  {
    auto *w = new QWidget; auto *l = new QVBoxLayout(w); auto *b = new QPushButton("x"); l->addWidget(b);
    uint64_t hb = T.add(b); int second = 0;
    QObject::connect(b, &QPushButton::clicked, [&] { T.destroy(hb); });
    QObject::connect(b, &QPushButton::clicked, [&] { ++second; });
    w->show(); flush();
    QTest::mouseClick(b, Qt::LeftButton);
    CHECK(second == 1);
    CHECK(!T.live(hb));  // handle gone from table immediately
    flush();
    delete w;
  }
  // (b) synchronous delete of the sender inside clicked(): Qt stops delivering to later slots
  {
    auto *w = new QWidget; auto *l = new QVBoxLayout(w); auto *b = new QPushButton("x"); l->addWidget(b);
    int second = 0; QPointer<QPushButton> g(b);
    QObject::connect(b, &QPushButton::clicked, [&] { delete b; });
    QObject::connect(b, &QPushButton::clicked, [&] { ++second; });
    w->show(); flush();
    QTest::mouseClick(b, Qt::LeftButton);   // run under valgrind to see invalid reads
    std::printf("  info sync-delete: later slot ran=%d, guard null=%d\n", second, g.isNull());
    flush();
    delete w;
  }
  // (c) deleting the parent window from a child's signal: deleteLater only
  {
    auto *w = new QWidget; auto *l = new QVBoxLayout(w); auto *b = new QPushButton("x"); l->addWidget(b);
    uint64_t hw = T.add(w), hb = T.add(b);
    QObject::connect(b, &QPushButton::clicked, [&] { T.destroy(hw); });
    w->show(); flush();
    QTest::mouseClick(b, Qt::LeftButton);
    flush();
    CHECK(!T.live(hw) && !T.live(hb));   // b died with w; b's on_destroyed fired
    dumplog(T);
  }
}

// 5. destroyed timing: children vs parent, and object state during `destroyed`
static void case_destroyed_timing() {
  std::printf("CASE destroyed_timing\n");
  Table T;
  auto *w = new QWidget; auto *l = new QVBoxLayout(w); auto *b = new QPushButton("x"); l->addWidget(b);
  auto *act = new QAction("a", w);  // non-widget QObject child
  T.add(w); T.add(b); T.add(act); T.add(l);
  delete w;
  dumplog(T);
}

int main(int argc, char **argv) {
  QApplication app(argc, argv);
  std::printf("info quitOnLastWindowClosed default=%d\n", (int)app.quitOnLastWindowClosed());
  app.setQuitOnLastWindowClosed(false);
  case_detach_reinsert();
  case_native_close();
  case_borrowed();
  case_delete_in_signal();
  case_destroyed_timing();
  std::printf("FAILURES %d\n", g_case_fail);
  return g_case_fail ? 1 : 0;
}
