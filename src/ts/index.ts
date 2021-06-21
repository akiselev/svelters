//// <reference path="./types.d.ts" />

import App from "./App.svelte";
console.log(".....");
setTimeout(() => console.log("Finished"), 8000);
new App({ });
// new App({ target: {} });
