import * as core from "Core";
import * as widgets from "Widgets";

import { Object, Application } from "Core";
import { Widget, MainWindow, BoxLayout, PushButton } from "Widgets";

const classes = {
    Object,
    Application,
    Widget,
    MainWindow,
    BoxLayout,
    PushButton,
};

export const constructClass = (cls = "Object", ...args) => {
    let Constructor = classes[cls];
    if (typeof Constructor !== "function") {
        throw new Error(`Type ${cls} has no constructor`);
    }
    let obj = new Constructor(...args);
    return obj;
};

export const getClassName = (cls = "Object") => {
    return constructClass(cls).className();
};

export const getTypeOf = (cls = "Object", ...args) => {
    let target = classes[cls];
    if (args.length > 0) {
        target = new target();
    }
    for (const arg in args) {
        if (typeof target[arg] === "undefined") {
            return "undefined";
        }
        target = target[arg];
    }
    return typeof target;
};

export const callMethod = (cls, name = "className", ...args) => {
    let inst = constructClass(cls);
    return inst[name](...args);
};

export const hasMethod = (cls, name = "className") => {
    let inst = constructClass(cls);
    return typeof inst[name] === "function";
};

export const callStatic = (cls, name = "staticFn", ...args) => {
    let Class = classes[cls];
    return Class[name](...args);
};

export const hasStatic = (cls, name = "staticFn") => {
    let Class = classes[cls];
    return typeof Class[name] === "function";
};

export const typeofProperty = (cls, name = "className") => {
    let inst = constructClass(cls);
    return typeof inst[name];
};

export const getProperty = (cls, name) => {
    let inst = constructClass(cls);
    return inst[name];
};

export const setProperty = (cls, name, value) => {
    let inst = constructClass(cls);
    console.log(`>> set ${cls}::${name} = ${value}`);
    inst[name] = value;
    const newValue = inst[name];
    console.log(`>> get ${cls}::${name} = ${newValue}`);
    return newValue;
};

export const callMethodAfterCast = (
    cls,
    castTo = "Object",
    name = "className",
    ...args
) => {
    let inst = constructClass(cls);
    inst = inst.castAs(castTo);
    return inst[name](...args);
};

export const callElementMethodAfterCast = (
    el,
    castTo = "Object",
    name = "className",
    ...args
) => {
    let inst = document.createElement(el);
    inst = inst.castAs(castTo);
    return inst[name](...args);
};

export const getElementClassName = (el = "object") => {
    return document.createElement(el).className();
};

export const callElementMethod = (el, name = "className", ...args) => {
    let inst = document.createElement(el);
    return inst[name](...args);
};

export const getElementProp = (el, name = "className") => {
    let inst = document.createElement(el);
    return inst[name];
};

export const getElementProperty = getElementProp;

export const setElementProperty = (el, name, value) => {
    let inst = document.createElement(el);
    inst[name] = value;
    const newValue = inst[name];
    return newValue;
};

export const addWidget = (cls) => {
    let box = constructClass("BoxLayout");
    let inst = constructClass(cls);
    return box.addWidget(inst);
};

export const addObject = (cls) => {
    let box = constructClass("BoxLayout");
    let inst = constructClass(cls);
    return box.addObject(inst);
};
