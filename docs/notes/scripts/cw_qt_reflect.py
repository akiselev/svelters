#!/usr/bin/env python3
"""Probe: how much of a QWidget subclass is reachable by name with no codegen (custom-widgets research).

Usage: QT_QPA_PLATFORM=offscreen python3 -I docs/notes/scripts/cw_qt_reflect.py
Checks the three Qt "create by class name" routes and what QMetaObject exposes afterwards.
"""
from PySide6.QtCore import QMetaType, QMetaMethod
from PySide6.QtWidgets import QApplication
from PySide6.QtUiTools import QUiLoader

app = QApplication([])
ui = QUiLoader()
names = ui.availableWidgets()
print(f"QUiLoader.availableWidgets(): {len(names)} classes (built-ins + designer plugins on QT_PLUGIN_PATH/designer)")
print("  plugin paths:", ui.pluginPaths())

w = ui.createWidget("QPushButton")
mo = w.metaObject()
print(f"createWidget('QPushButton') -> {type(w).__name__}; metaObject {mo.className()} "
      f"props {mo.propertyCount()} methods {mo.methodCount()} ctors(Q_INVOKABLE) {mo.constructorCount()}")
sigs = [bytes(mo.method(i).methodSignature()).decode() for i in range(mo.methodCount())
        if mo.method(i).methodType() == QMetaMethod.MethodType.Signal]
print("  signals:", sigs[:12], "...")
print("  writable props:", [mo.property(i).name() for i in range(mo.propertyCount()) if mo.property(i).isWritable()][:15], "...")

for n in ("QPushButton*", "QPushButton", "QWidget*"):
    mt = QMetaType.fromName(n.encode())
    print(f"QMetaType.fromName({n!r}): valid={mt.isValid()} metaObject={'yes' if mt.isValid() and mt.metaObject() else 'no'}")
print("unknown class:", ui.createWidget("ads::CDockManager"), "| errorString:", ui.errorString())
