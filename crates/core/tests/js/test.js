import * as core from "Core";
import * as widgets from "Widgets";

// import { Object } from "Core";

const classes = { ...core, ...widgets };

export const constructClass = (name = "Object") => {
    let Constructor = classes[name];
    return new Constructor();
};

export const getClassName = (name = "Object") => {
    return constructClass(name).className();
};

export const testReturnValue = (val = "World") => `Hello ${val}`;

export const testConsole = (val = "Hello World") => {
    // const obj = new Object();
    console.log("Test:", val);
    // throw new Error("test");
};

export const callMethod = (cls = "Object", name = "className", ...args) => {
    let inst = constructClass(cls);
    return inst[name](...args);
};

export const testArgs = (...args) => {
    return args.join(" ");
};
