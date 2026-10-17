#!/usr/bin/env python3
"""Probe Qt sync-result semantics that the runtime design depends on.

1. Close veto through an event filter: does `return True` alone veto, or must the filter
   also call event.ignore()? (QCloseEvent starts accepted.)
2. Key event through a filter: return True stops delivery to keyPressEvent.
3. Echo suppression: QLineEdit.setText emits textChanged (not textEdited); QSignalBlocker
   suppresses it; setting an unchanged value does not emit for QSpinBox.valueChanged.
4. Re-entrancy: deleting / reparenting a widget from inside its own event filter.

Usage: QT_QPA_PLATFORM=offscreen python3 -I qt_sync_probe.py
"""
from PySide6.QtCore import QObject, QEvent, QSignalBlocker, Qt
from PySide6.QtGui import QKeyEvent
from PySide6.QtWidgets import QApplication, QWidget, QLineEdit, QSpinBox, QVBoxLayout, QLabel

app = QApplication([])


class Filter(QObject):
    def __init__(self, fn):
        super().__init__()
        self.fn = fn

    def eventFilter(self, obj, ev):
        return self.fn(obj, ev)


def close_case(ignore, ret):
    w = QWidget()
    w.show()
    def fn(o, e):
        if e.type() == QEvent.Close:
            if ignore:
                e.ignore()
            return ret
        return False
    f = Filter(fn)
    w.installEventFilter(f)
    r = w.close()
    print(f"close: filter ignore={ignore!s:5} return={ret!s:5} -> close() returned {r}, visible={w.isVisible()}")
    w.removeEventFilter(f)
    w.hide()


for ig in (False, True):
    for rt in (False, True):
        close_case(ig, rt)


class KeyW(QLineEdit):
    got = 0
    def keyPressEvent(self, e):
        KeyW.got += 1
        super().keyPressEvent(e)


k = KeyW()
f = Filter(lambda o, e: e.type() == QEvent.KeyPress and e.key() == Qt.Key_A)
k.installEventFilter(f)
for key, txt in ((Qt.Key_A, "a"), (Qt.Key_B, "b")):
    QApplication.sendEvent(k, QKeyEvent(QEvent.KeyPress, key, Qt.NoModifier, txt))
print(f"key filter: keyPressEvent ran {KeyW.got}x for A(filtered)+B ; text={k.text()!r}")

le = QLineEdit()
log = []
le.textChanged.connect(lambda t: log.append(("textChanged", t)))
le.textEdited.connect(lambda t: log.append(("textEdited", t)))
le.setText("x")
with QSignalBlocker(le):
    le.setText("y")
le.setText("y")
print("QLineEdit setText x, blocked y, same y ->", log)

sb = QSpinBox()
slog = []
sb.valueChanged.connect(slog.append)
sb.setValue(5); sb.setValue(5); sb.setValue(500)
print("QSpinBox setValue 5,5,500 (max 99) ->", slog, "value", sb.value())

# re-entrancy: delete the watched widget inside its filter
parent = QWidget(); lay = QVBoxLayout(parent); child = QLabel("c"); lay.addWidget(child)
def kill(o, e):
    if e.type() == QEvent.MouseButtonPress:
        o.deleteLater()            # safe: deferred
        return True
    return False
f2 = Filter(kill); child.installEventFilter(f2)
from PySide6.QtGui import QMouseEvent
from PySide6.QtCore import QPointF
QApplication.sendEvent(child, QMouseEvent(QEvent.MouseButtonPress, QPointF(1, 1), QPointF(1, 1),
                                          Qt.LeftButton, Qt.LeftButton, Qt.NoModifier))
app.processEvents(); QApplication.sendPostedEvents(None, QEvent.DeferredDelete)
print("deleteLater inside filter: layout count after =", lay.count())
