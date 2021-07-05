import * as core from "Core";
import * as widgets from "Widgets";

// import { Object } from "Core";

const classes = { ...core, ...widgets };

export const constructClass = (name = "Object") => {
    let Constructor = classes[name];
    return new Constructor();
};

export const testReturnValue = (val = "World") => `Hello ${val}`;

export const testConsole = (val = "Hello World") => {
    // const obj = new Object();
    console.log("Test:", val);
    // throw new Error("test");
};
