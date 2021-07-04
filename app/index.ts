//// <reference path="./types.d.ts" />

import * as mod from "QtCore";

console.log(">>>>>1", mod);
console.log(">>>>>2", mod.QObject);
let obj = new mod.QObject();
console.log(">>>>>3", obj);
window.main = new mod.QMainWindow();
console.log(">>>>>4", window.main);
console.log(">>>>>5", window.main.show);
console.log(">>>>>6", typeof window.main);
window.main.show();

// import App from "./App.svelte";

// window = document.createElement("qMainWindow");

// console.log(".....");
// setTimeout(() => console.log("Timeout 8sec"), 8000);

// window.app = new App({ target: window });

// window.callMethod("show");