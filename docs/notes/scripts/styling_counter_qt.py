# Counter styled through the Qt path: translated QSS template + tokens resolved from QPalette by
# the "host", class string as a dynamic property, inline style as a per-widget sheet.
import json, sys
from PySide6.QtWidgets import QApplication, QMainWindow, QWidget, QVBoxLayout, QLabel, QPushButton
from PySide6.QtGui import QPalette, QColor
from PySide6.QtCore import Qt, QPoint
from PySide6.QtTest import QTest
S = sys.argv[1]; T = json.load(open(f'{S}/counter_translated.json')); H = T['hash']
app = QApplication(['counter']); print('style', app.style().name())
def tokens(dark):
    p = app.palette(); acc = p.color(QPalette.ColorRole.Accent)
    t = {'--sv-accent': acc.name(), '--sv-accent-hover': acc.lighter(115).name(), '--sv-accent-fg': '#ffffff',
         '--sv-surface': p.color(QPalette.ColorRole.Base).name(), '--sv-border': p.color(QPalette.ColorRole.Mid).name(),
         '--sv-fg': p.color(QPalette.ColorRole.WindowText).name()}
    if dark: t.update({'--sv-surface': '#2a2a2e', '--sv-border': '#444444', '--sv-fg': '#eeeeec'})
    return t
def render_sheet(dark):   # host-side: substitute ${--token} placeholders, append dark variant
    css = T['qt']['css'] + ('\n' + T['qt']['dark'] if dark else '')
    for k, v in tokens(dark).items(): css = css.replace('${' + k + '}', v)
    if dark: css += '\n.QMainWindow { background: #1e1e20; }'
    return css
app.setStyleSheet(render_sheet(False))
win = QMainWindow(); win.setWindowTitle('Counter (Qt)')
box = QWidget(); lay = QVBoxLayout(box); lay.setSpacing(8); lay.setContentsMargins(12, 12, 12, 12)
lbl = QLabel('Count: 0'); lbl.setAlignment(Qt.AlignmentFlag.AlignCenter); btn = QPushButton('Increment')
for w, c in ((box, f'card {H}'), (lbl, f'count {H}'), (btn, f'primary {H}')): w.setProperty('class', f'{w.metaObject().className()} {c}')
lay.addWidget(lbl); lay.addWidget(btn)
outer = QWidget(); ol = QVBoxLayout(outer); ol.setContentsMargins(12, 12, 12, 12); ol.addWidget(box)
win.setCentralWidget(outer); win.resize(260, 150); win.show()
def inline(w, decls): w.setStyleSheet(f'.{w.metaObject().className()} {{ {decls} }}')   # scoped to the widget itself
inline(lbl, 'font-weight: bold;')
def shot(name):
    app.processEvents(); win.grab().save(f'{S}/shots/qt_{name}.png')
    print(f'{name:10s} label fg={lbl.palette().color(lbl.foregroundRole()).name()}')
QTest.qWait(100); shot('normal')
QTest.mouseMove(btn, QPoint(20, 10)); QTest.qWait(100); shot('hover'); QTest.mouseMove(win, QPoint(2, 2)); QTest.qWait(50)
lbl.setText('Count: 3'); inline(lbl, 'font-weight: bold; color: #c01c28;'); shot('inline')
app.styleHints().setColorScheme(Qt.ColorScheme.Dark); QTest.qWait(50)
print('colorScheme now', app.styleHints().colorScheme(), 'window bg', app.palette().color(QPalette.ColorRole.Window).name())
app.setStyleSheet(render_sheet(True)); shot('dark')
