# Which QSS selectors/values/properties take effect? Each case styles one QPushButton inside a
# QWidget parent; we render and check whether the button's centre pixel became red.
import sys, json
from PySide6.QtCore import qInstallMessageHandler
from PySide6.QtWidgets import QApplication, QWidget, QPushButton, QLabel, QVBoxLayout
from PySide6.QtGui import QColor
msgs = []
qInstallMessageHandler(lambda mode, ctx, m: msgs.append(m))
app = QApplication(['probe'])
RED = 'background: rgb(255,0,0); border: none;'
def probe(sheet, setup=None, where='app', state=None):
    msgs.clear()
    root = QWidget(); root.setObjectName('root'); lay = QVBoxLayout(root)
    lbl = QLabel('l'); btn = QPushButton('button'); lay.addWidget(lbl); lay.addWidget(btn)
    if setup: setup(root, lbl, btn)
    app.setStyleSheet(''); root.setStyleSheet('')
    (app if where == 'app' else root).setStyleSheet(sheet)
    root.resize(200, 100); root.show(); app.processEvents()
    if state == 'hover':
        from PySide6.QtCore import Qt
        btn.setAttribute(Qt.WA_UnderMouse, True); btn.update()
    if state == 'checked': btn.setCheckable(True); btn.setChecked(True)
    if state == 'disabled': btn.setEnabled(False)
    app.processEvents()
    img = btn.grab().toImage(); c = QColor(img.pixel(5, img.height() // 2))
    root.close(); root.deleteLater(); app.processEvents()
    return ('APPLIED' if (c.red() > 200 and c.green() < 60) else 'no effect') + (f'  [{"; ".join(set(msgs))}]' if msgs else '')
def cls(v): return lambda r, l, b: b.setProperty('class', v)
cases = {
 'type QPushButton': (f'QPushButton {{{RED}}}', None),
 'type QAbstractButton (base class)': (f'QAbstractButton {{{RED}}}', None),
 '.QPushButton (exact class)': (f'.QPushButton {{{RED}}}', None),
 '.QAbstractButton (exact, base)': (f'.QAbstractButton {{{RED}}}', None),
 '#objectName': (f'#ok {{{RED}}}', lambda r, l, b: b.setObjectName('ok')),
 '#obj-with-dash': (f'#sv-1 {{{RED}}}', lambda r, l, b: b.setObjectName('sv-1')),
 '[class="a"] string': (f'[class="a"] {{{RED}}}', cls('a')),
 '[class~="b"] space-separated string "a b c"': (f'[class~="b"] {{{RED}}}', cls('a b c')),
 '[class~="b"] QStringList': (f'[class~="b"] {{{RED}}}', cls(['a', 'b'])),
 '[class="b"] QStringList': (f'[class="b"] {{{RED}}}', cls(['a', 'b'])),
 '[class|="b"]': (f'[class|="b"] {{{RED}}}', cls('b-x')),
 '[svelte-abc="true"] bool prop w/ dash': (f'[svelte-abc="true"] {{{RED}}}', lambda r, l, b: b.setProperty('svelte-abc', True)),
 '[svelte_abc="true"]': (f'[svelte_abc="true"] {{{RED}}}', lambda r, l, b: b.setProperty('svelte_abc', True)),
 '[flag] presence': (f'[flag] {{{RED}}}', lambda r, l, b: b.setProperty('flag', 'x')),
 '[text="button"] (Q_PROPERTY)': (f'QPushButton[text="button"] {{{RED}}}', None),
 'descendant': (f'#root QPushButton {{{RED}}}', None),
 'child >': (f'#root > QPushButton {{{RED}}}', None),
 'adjacent +': (f'QLabel + QPushButton {{{RED}}}', None),
 'sibling ~': (f'QLabel ~ QPushButton {{{RED}}}', None),
 ':hover': (f'QPushButton:hover {{{RED}}}', None, 'hover'),
 ':checked': (f'QPushButton:checked {{{RED}}}', None, 'checked'),
 ':disabled': (f'QPushButton:disabled {{{RED}}}', None, 'disabled'),
 ':!checked (negation)': (f'QPushButton:!checked {{{RED}}}', None),
 ':not(:checked)': (f'QPushButton:not(:checked) {{{RED}}}', None),
 ':where([class~="a"])': (f'QPushButton:where([class~="a"]) {{{RED}}}', cls('a')),
 ':first-child': (f'QPushButton:first-child {{{RED}}}', None),
 '* universal': (f'* {{{RED}}}', None),
 'var() custom prop': ('QPushButton { --c: red; background: var(--c); border: none; }', None),
 'palette(highlight) value': ('QPushButton { background: palette(highlight); border: none; }', lambda r, l, b: (lambda p: (p.setColor(p.ColorRole.Highlight, QColor('red')), app.setPalette(p)))(app.palette())),
 'rgba() / hsl()': ('QPushButton { background: hsl(0, 100%, 50%); border: none; }', None),
 'oklch()': ('QPushButton { background: oklch(0.63 0.26 29); border: none; }', None),
 'background only (no border)': ('QPushButton { background: red; }', None),
 'background-color only (no border)': ('QPushButton { background-color: red; }', None),
 '@media wrapper': (f'@media (prefers-color-scheme: dark) {{ QPushButton {{{RED}}} }} QPushButton {{{RED}}}', None),
 'unknown property alongside': (f'QPushButton {{ display: flex; {RED} }}', None),
 'transition alongside': (f'QPushButton {{ transition: color 1s; {RED} }}', None),
 '!important': ('QPushButton { background: red !important; border: none; }', None),
 'syntax error elsewhere in sheet': (f'QPushButton {{{RED}}} QLabel {{ color: red', None),
 'nesting &': (f'#root {{ & QPushButton {{{RED}}} }}', None),
 'widget-level sheet, no selector': (f'{RED}', lambda r, l, b: b.setStyleSheet(RED), 'none'),
 'parent sheet descendant': (f'QPushButton {{{RED}}}', None, None, 'root'),
}
out = {}
for name, spec in cases.items():
    sheet, setup = spec[0], spec[1]; state = spec[2] if len(spec) > 2 else None; where = spec[3] if len(spec) > 3 else 'app'
    if state == 'none': state = None; sheet = ''
    out[name] = probe(sheet, setup, where, state)
    print(f'{name:48s} {out[name]}')
