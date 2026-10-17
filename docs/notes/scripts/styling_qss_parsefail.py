from PySide6.QtWidgets import QApplication, QWidget, QPushButton, QLabel, QVBoxLayout
from PySide6.QtGui import QColor
app = QApplication(['p'])
root = QWidget(); lay = QVBoxLayout(root); lbl = QLabel('label'); btn = QPushButton('b'); lay.addWidget(lbl); lay.addWidget(btn); root.show()
def fg(w): return w.palette().color(w.foregroundRole()).name()
for bad in ['QLabel { --c: red; }', 'QLabel { color: var(--c); }', '@media (x) { QLabel { color: red; } }', 'QLabel:not(:hover) { color: red; }',
            'QLabel:where(.x) { color: red; }', 'QLabel { display: flex; }', 'QLabel { color: oklch(1 0 0); }', 'QLabel + QPushButton { color: red; }']:
    sheet = 'QLabel { color: #00ff00; } ' + bad + ' QPushButton { color: #0000ff; }'
    app.setStyleSheet(''); app.processEvents(); app.setStyleSheet(sheet); app.processEvents()
    print(f'{bad:42s} before-rule label={fg(lbl)}  after-rule button={fg(btn)}')
