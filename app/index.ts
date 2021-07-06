//// <reference path="./types.d.ts" />

import * as mod from "QtCore";

console.log(">>>>>1", mod);
console.log(">>>>>2", mod.QObject);
let obj = new mod.QObject();
console.log(">>>>>3", obj.staticTest);
mod.QObject.staticTest && mod.QObject.staticTest();
console.log(">>>>>4.1");
obj.test && obj.test();
console.log(">>>>>4.2");
let widget = new mod.QWidget();
widget.test ? widget.test(): console.log("FAILED WIDGET TEST");
console.log(">>>>>4.3");
mod.QWidget.staticTest ? mod.QWidget.staticTest() : console.log("FAILED");
console.log(">>>>>4.4");
window.main = new mod.QMainWindow();
console.log(">>>>>5", window.main.show);
console.log(">>>>>6", typeof window.main);
console.log(">>>>>>>>>>>>>>>>>>>>>>>>>>7", obj.className());
console.log(">>>>>>>>>>>>>>>>>>>>>>>>>>7", window.main.className());
try {
    window.main.show();
} catch (err) {
    console.log(">>>1", err);
    console.log(">>>1", err.message);
    console.log(">>>1", err.stack);
    console.log(">>>1", err.rustStack);
}

// import App from "./App.svelte";

// window = document.createElement("qMainWindow");

// console.log(".....");
// setTimeout(() => console.log("Timeout 8sec"), 8000);

// window.app = new App({ target: window });

// window.callMethod("show");