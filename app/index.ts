/// <reference path="./dom/dom.d.ts" />

import "./dom/document.ts";
import App from "./App.svelte";

const start = () => {
    console.log(">>>>>, 1");
    window.appWindow = new ApplicationWindow();
    window.appWindow.hideOnClose = true;
    window.titlebar = new Button();
    window.appWindow.titlebar = window.titlebar;
    window.appWindow.titlebar.label = "TITLEBAR";
    window.titlebar.connect_clicked(() => {
        console.log("Titlebar clicked!");
    });
    console.log(">>>>>, 2");
    const app = new App({ target: window.appWindow });
    console.log(">>>>>, 3");
    window.appWindow.show();
    console.log(">>>>>, 4");
    return app;
};

export default start;

// console.log(">>>", "keyx".matchAll(/key/));

// window.insertBefore

// import * as core from "Gtk";

//--------------------------------
// import * as mod from "QtCore";
// window.main = document.createElement("qMainWindow");
// const btn = document.createElement("qPushButton");
// btn.text = "Hello World";
// console.log(">>>>", btn.castAs("QWidget"));
// window.main.appendChild(btn);
// window.main.show();
//--------------------------------
// import * as mod from "QtCore";
// window.main = new mod.QMainWindow();
// console.log("111");
// const button = new mod.QPushButton();
// console.log("222");
// button.text = "TEST TEXT";
// console.log(window.main.setCentralWidget);
// window.main.setCentralWidget(button);
// console.log("333");
// window.main.show();
// console.log("444");
//--------------------------------
// import { QMainWindow } from "QtCore";

// import App from "./App.svelte";

// window.main = new QMainWindow();

// window.main.App = new App({ target: window.main });
// window.main.show();
