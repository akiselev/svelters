import SvelteQtDocument from "./document";
import { loadBinding } from "@node-rs/helper";

//expose our fake dom as global document for svelte components
let window = global as any;

window.window = global;

const doc = new SvelteQtDocument();

Object.defineProperty(window, "document", {
    value: doc,
    enumerable: true,
    configurable: false,
    writable: false,
});

Object.defineProperty(global, "document", {
    value: doc,
    enumerable: true,
    configurable: false,
    writable: false,
});

/**
 * __dirname means load native addon from current dir
 * 'svelte-qt' means native addon name is `svelte-qt`
 * the first arguments was decided by `napi.name` field in `package.json`
 * the second arguments was decided by `name` field in `package.json`
 * loadBinding helper will load `svelte-qt.[PLATFORM].node` from `__dirname` first
 * If failed to load addon, it will fallback to load from `svelte-qt-[PLATFORM]`
 */
console.log("Loading rust library....");
const { QMainWindow, QApplication, QPushButton } = loadBinding(
    __dirname,
    "svelte-qt",
    "svelte-qt",
);
console.log("Creating application....");
const app = new QApplication();
app.exec();

console.log("Creating main window....");
const mainWindow = new QMainWindow();
// const button = new QPushButton();
// button.onClicked((res: boolean) => console.log("Clicked!!!", res));
// mainWindow.setCentralWidget(button);

console.log("Showing window....");
mainWindow.show();

setTimeout(() => console.log("Finished"), 5000);
