//// <reference path="./types.d.ts" />

import App from "./App.svelte";
// import SvelteQtDocument from "./document";
// import { QMainWindow, QApplication, QPushButton } from "./widgets";

// //expose our fake dom as global document for svelte components
// let window = global as any;

// window.window = global;

// const doc = new SvelteQtDocument();

// Object.defineProperty(window, "document", {
//     value: doc,
//     enumerable: true,
//     configurable: false,
//     writable: false,
// });

// Object.defineProperty(global, "document", {
//     value: doc,
//     enumerable: true,
//     configurable: false,
//     writable: false,
// });

// console.log("Creating application....");
// const app = new QApplication();
// app.exec();

// console.log("Creating main window....");
// const mainWindow = new QMainWindow();
// // const button = new QPushButton();
// // button.onClicked((res: boolean) => console.log("Clicked!!!", res));
// // mainWindow.setCentralWidget(button);

// const svApp = new App({ target: mainWindow, props: {} });

// console.log("Showing window....");
// mainWindow.show();
new App({ target: this });
setTimeout(() => console.log("Finished"), 5000);
