#include <QtCore/QString>
#include <QtCore/QByteArray>
#include <chrono>
#include <cstdio>
#include <string>
static volatile qsizetype sink;
template <class F> double bench(F f, int n) {
  double best = 1e18;
  for (int r = 0; r < 7; r++) {
    auto t0 = std::chrono::steady_clock::now();
    for (int i = 0; i < n; i++) f();
    double ns = std::chrono::duration<double, std::nano>(std::chrono::steady_clock::now() - t0).count() / n;
    if (ns < best) best = ns;
  }
  return best;
}
int main() {
  struct Case { const char *name; QString s; };
  Case cases[] = {{"11B ascii", QStringLiteral("hello world")},
                  {"1Ki ascii", QString(1024, QChar('x'))},
                  {"1Ki latin1", QString(1024, QChar(0xe9))},
                  {"1Ki wide", QString(512, QChar(0xe9)) + QString(512, QChar(0x2713))}};
  for (auto &c : cases) {
    QByteArray u8 = c.s.toUtf8();
    std::u16string u16(reinterpret_cast<const char16_t *>(c.s.utf16()), c.s.size());
    double a = bench([&] { QString q = QString::fromUtf8(u8.constData(), u8.size()); sink = q.size(); }, 100000);
    double b = bench([&] { QString q = QString::fromUtf16(u16.data(), u16.size()); sink = q.size(); }, 100000);
    double d = bench([&] { QByteArray o = c.s.toUtf8(); sink = o.size(); }, 100000);
    std::printf("%-12s fromUtf8 %7.1f ns   fromUtf16 %7.1f ns   toUtf8 (out) %7.1f ns\n", c.name, a, b, d);
  }
}
