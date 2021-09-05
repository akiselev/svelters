import * as mod from "QtCore";
window.main = new mod.QMainWindow();
console.log("111");
const button = new mod.QPushButton();
console.log("222");
button.text = "TEST TEXT";
console.log(window.main.setCentralWidget);
window.main.setCentralWidget(button);
console.log("333");
window.main.show();
console.log("444");

// import { QMainWindow } from "QtCore";

// import App from "./App.svelte";

// window.main = new QMainWindow();

// window.main.App = new App({ target: window.main });
// window.main.show();
