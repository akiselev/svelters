#!/usr/bin/env python3
"""Probe Qt single-slot setters: default child, replace semantics, clear-with-null.

For each (class, getter, setter, child factory):
  * default : what the getter returns on a fresh instance (built-in sub-widget? lazily created?)
  * replace : set A, then set B -> is A deleted (shiboken6.isValid after DeferredDelete),
              re-parented, or left as-is?
  * clear   : set None -> accepted? what does the getter return afterwards (a fresh default?)
  * owner   : parent of a child after set

Also probes the multi-slot remove ops (removeTab/removeWidget/removeRow/takeRow/...).

Usage: QT_QPA_PLATFORM=offscreen python3 -I qt_slot_ownership_probe.py
"""
import shiboken6
from PySide6.QtCore import QEvent, QStringListModel, Qt
from PySide6.QtGui import QAction, QStandardItemModel
from PySide6.QtWidgets import (
    QAbstractItemView, QApplication, QComboBox, QDockWidget, QFormLayout, QGridLayout, QHeaderView,
    QLabel, QLineEdit, QListView, QMainWindow, QMenu, QMenuBar, QPushButton, QScrollArea, QScrollBar,
    QSplitter, QStackedWidget, QStatusBar, QStyledItemDelegate, QTableView, QTabBar, QTabWidget,
    QToolBar, QToolBox, QToolButton, QVBoxLayout, QWidget, QWizard, QWizardPage, QTreeView, QGroupBox)

app = QApplication([])


def flush():
    app.processEvents()
    QApplication.sendPostedEvents(None, QEvent.DeferredDelete)


def desc(o, host):
    if o is None:
        return "None"
    if not shiboken6.isValid(o):
        return "DELETED"
    p = o.parent() if hasattr(o, "parent") else None
    rel = "child-of-host" if p is host else ("unparented" if p is None else f"parent={type(p).__name__}")
    return f"{type(o).__name__}({rel})"


def single(host_f, get, set_, make, note="", clear=True):
    h = host_f()
    g = getattr(h, get)
    s = getattr(h, set_)
    default = g()
    d0 = desc(default, h)
    a = make()
    s(a)
    flush()
    old_default = desc(default, h) if default is not None and default is not a else "-"
    b = make()
    s(b)
    flush()
    ra = desc(a, h)
    if not clear:
        cleared = "not probed (segfaults: no null check)"
    else:
      try:
        s(None)
        flush()
        cleared = f"getter={desc(g(), h)} B={desc(b, h)}"
      except Exception as e:
        cleared = f"rejected ({type(e).__name__})"
    print(f"{type(h).__name__}.{set_:22} default={d0:28} default-after-set={old_default:14} "
          f"old-after-replace={ra:26} set(None): {cleared} {note}")


def editable_combo():
    c = QComboBox(); c.setEditable(True); return c


print("== single slots ==")
single(QMainWindow, "centralWidget", "setCentralWidget", QLabel)
single(QMainWindow, "menuBar", "setMenuBar", QMenuBar, "(menuBar() creates lazily)")
single(QMainWindow, "statusBar", "setStatusBar", QStatusBar, "(statusBar() creates lazily)")
single(QMainWindow, "menuWidget", "setMenuWidget", QLabel)
single(QDockWidget, "widget", "setWidget", QLabel)
single(QDockWidget, "titleBarWidget", "setTitleBarWidget", QLabel)
single(QScrollArea, "widget", "setWidget", QLabel)
single(QScrollArea, "viewport", "setViewport", QWidget)
single(QScrollArea, "cornerWidget", "setCornerWidget", QLabel)
single(QScrollArea, "horizontalScrollBar", "setHorizontalScrollBar", QScrollBar)
single(editable_combo, "lineEdit", "setLineEdit", QLineEdit)
single(QComboBox, "view", "setView", QListView)
single(QComboBox, "model", "setModel", lambda: QStringListModel(["a"]))
single(QComboBox, "itemDelegate", "setItemDelegate", QStyledItemDelegate)
single(QTableView, "horizontalHeader", "setHorizontalHeader", lambda: QHeaderView(Qt.Horizontal))
single(QTableView, "model", "setModel", QStandardItemModel)
single(QTableView, "itemDelegate", "setItemDelegate", QStyledItemDelegate)
single(QTreeView, "header", "setHeader", lambda: QHeaderView(Qt.Horizontal))
single(QTabWidget, "tabBar", "setTabBar", QTabBar, clear=False)
single(QTabWidget, "cornerWidget", "setCornerWidget", QLabel)
single(QToolButton, "menu", "setMenu", QMenu)
single(QPushButton, "menu", "setMenu", QMenu)
single(QLabel, "buddy", "setBuddy", QLineEdit, "(reference, not ownership)")

print("== index widget ==")
tv = QTableView(); m = QStandardItemModel(2, 2); tv.setModel(m)
a, b = QLabel("a"), QLabel("b")
tv.setIndexWidget(m.index(0, 0), a); flush(); tv.setIndexWidget(m.index(0, 0), b); flush()
print("QTableView.setIndexWidget replace: old", desc(a, tv.viewport()), "new", desc(b, tv.viewport()))

print("== multi-slot removal ==")
tw = QTabWidget(); p = QLabel(); tw.addTab(p, "t"); tw.removeTab(0); flush()
print("QTabWidget.removeTab -> page", desc(p, tw))
st = QStackedWidget(); p = QLabel(); st.addWidget(p); st.removeWidget(p); flush()
print("QStackedWidget.removeWidget -> page", desc(p, st))
tb = QToolBox(); p = QLabel(); tb.addItem(p, "x"); tb.removeItem(0); flush()
print("QToolBox.removeItem -> page", desc(p, tb))
sp = QSplitter(); a, b = QLabel(), QLabel(); sp.addWidget(a); old = sp.replaceWidget(0, b); flush()
print("QSplitter.replaceWidget -> old", desc(a, sp), "returned-is-old", old is a, "count", sp.count())
wz = QWizard(); pg = QWizardPage(); pid = wz.addPage(pg); wz.removePage(pid); flush()
print("QWizard.removePage -> page", desc(pg, wz))
host = QWidget(); fl = QFormLayout(host); l1, f1 = QLabel("l"), QLineEdit(); fl.addRow(l1, f1); fl.removeRow(0); flush()
print("QFormLayout.removeRow -> label", desc(l1, host), "field", desc(f1, host))
l2, f2 = QLabel("l"), QLineEdit(); fl.addRow(l2, f2); r = fl.takeRow(0); flush()
print("QFormLayout.takeRow -> label", desc(l2, host), "field", desc(f2, host))
host = QWidget(); vl = QVBoxLayout(host); c = QLabel(); vl.addWidget(c); vl.removeWidget(c); flush()
print("QBoxLayout.removeWidget -> child", desc(c, host), "visible-flag", c.isVisibleTo(host))
mw = QMainWindow(); t = QToolBar(); mw.addToolBar(Qt.LeftToolBarArea, t); area = mw.toolBarArea(t)
mw.removeToolBar(t); flush()
print("QMainWindow.addToolBar(area) read back", area, "; removeToolBar -> toolbar", desc(t, mw))
d = QDockWidget(); mw.addDockWidget(Qt.RightDockWidgetArea, d); area = mw.dockWidgetArea(d); mw.removeDockWidget(d); flush()
print("QMainWindow.addDockWidget(area) read back", area, "; removeDockWidget -> dock", desc(d, mw))
le = QLineEdit(); act = QAction("x"); le.addAction(act, QLineEdit.TrailingPosition)
print("QLineEdit.addAction(pos): action parent", desc(act, le), "actions", len(le.actions()))
w = QWidget(); QVBoxLayout(w)
import io, contextlib
try:
    w.setLayout(QGridLayout())
    print("QWidget.setLayout when one exists: layout is", type(w.layout()).__name__)
except Exception as e:
    print("setLayout error", e)
w = QWidget(); c = QLabel()
print("QWidget without layout: layout() =", w.layout(), "(addWidget needs a layout first)")
gb = QGroupBox("g"); print("QGroupBox default layout =", gb.layout())
gh = QWidget(); gl = QGridLayout(gh); x = QLabel(); gl.addWidget(x, 1, 2, 2, 3)
print("QGridLayout.addWidget(w,1,2,2,3) read back getItemPosition =", gl.getItemPosition(gl.indexOf(x)))
