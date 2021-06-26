//// <reference path="./types.d.ts" />

import * as mod from "QtCore";

console.log(">>>>>", mod);
console.log(">>>>>", mod.QObject);
let obj = new mod.QObject();
console.log(">>>>>", obj);
console.log(">>>>>", JSON.stringify(Object.getOwnPropertyNames(obj)));

// import App from "./App.svelte";

// window = document.createElement("qMainWindow");

// console.log(".....");
// setTimeout(() => console.log("Timeout 8sec"), 8000);

// window.app = new App({ target: window });

// window.callMethod("show");