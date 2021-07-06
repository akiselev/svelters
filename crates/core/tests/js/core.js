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

export const setType = (cls, value) => {
    let inst = constructClass(cls);
    inst.type = value;
    return inst.type;
};
