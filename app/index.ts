//// <reference path="./types.d.ts" />

import App from "./App.svelte";

window = document.createElement("qMainWindow");

console.log(".....");
setTimeout(() => console.log("Timeout 8sec"), 8000);

window.app = new App({ target: window });

window.callMethod("show");