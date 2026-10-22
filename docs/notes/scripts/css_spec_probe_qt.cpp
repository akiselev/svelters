// Probe: does QSS count repeated class selectors (.a.a.a) in specificity?
#include <QApplication>
#include <QLabel>
#include <QPalette>
#include <cstdio>
static void probe(QApplication &app, const char *qss, const char *label) {
  app.setStyleSheet(QString::fromUtf8(qss));
  QLabel l("x");
  l.setProperty("class", "QLabel a b");
  l.show();
  l.ensurePolished();
  app.processEvents();
  QColor c = l.palette().color(QPalette::WindowText);
  std::printf("%-48s -> r=%d g=%d b=%d\n", label, c.red(), c.green(), c.blue());
}
int main(int argc, char **argv) {
  QApplication app(argc, argv);
  std::printf("Qt %s\n", qVersion());
  probe(app, ".a.a.a { color: rgb(255,0,0); } .a.b { color: rgb(0,0,255); }", "repeated .a.a.a then .a.b (red = counted)");
  probe(app, ".a.b { color: rgb(0,0,255); } .a { color: rgb(255,0,0); }", "control: .a.b then .a (blue expected)");
  probe(app, ".a { color: rgb(255,0,0); } .a { color: rgb(0,0,255); }", "control: equal spec, later wins (blue)");
  return 0;
}
