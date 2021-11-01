import * as nodes from "./nodes";
import { isStyle } from "./nodes";
import { Widget } from "./widget";
import { BoxWidget } from "./layout";
import * as window from "./window";

const ELEMENTS: { [name: string]: () => any } = {
    button: () => Widget("button", new Button()),
    hBox: () => BoxWidget("hBox", new HorizontalBox()),
    vBox: () => BoxWidget("vBox", new VerticalBox()),
};

document.root = new nodes.Node();

document.getRootNode = () => {
    return document;
};

document.createEvent = (type: string) => {
    console.log(`[Document] createEvent("${type}")`);
    let e: any = {};
    e.initCustomEvent = (
        type: string,
        ignored1: boolean,
        ignored2: boolean,
        detail: any,
    ) => {
        e.type = type;
        e.detail = detail;
        e.eventName = type;
    };
    return e;
};

document.createComment = (data: string): nodes.Comment => {
    console.log(`[Document] createComment("${data}")`);
    return new nodes.Comment(data);
};

document.createElement = (tagName: string): any => {
    let ele: any = null;
    console.log(`[Document] createElement("${tagName}")`);
    switch (tagName) {
        case "appWindow":
            ele = window.Window("appWindow", new ApplicationWindow());
            break;
        case "gtkWindow":
            ele = window.Window("gtkWindow", new Window());
            break;
        case "child":
            ele = new nodes.Child();
            break;
        case "style":
            ele = new nodes.Style();
            break;
        default:
            const func = ELEMENTS[tagName];
            if (func) {
                ele = func();
            } else {
                throw new Error(`No tag name "${tagName}" defined`);
            }
    }
    return ele;
};

document.createElementNS = (namespace: string, tagName: string): any => {
    return document.createElement(tagName);
};

document.createTextNode = (data: string): nodes.Text => {
    console.log(`[Document] createTextNode("${data}")`);
    return new nodes.Text(data);
};

document.styles = {};
document.host = {};
document.getElementById = (id: string): any | null => {
    return document.styles[id];
};

document.appendChild = (child: any) => {
    console.log(`[Document] appendChild("${child?.getAttribute("id")}")`);
    if (isStyle(child) && child.id) {
        child.provider.mount(0);
        document.styles[child.id] = child;
    } else {
        throw new Error(`Cannot append non-style children to document`);
    }
};

document.removeChild = (child: any) => {
    console.log(`[Document] removeChild("${child?.getAttribute("id")}")`);
    if (isStyle(child) && child.id) {
        document.styles[child.id]?.provider.unmount();
        delete document.styles[child.id];
    } else {
        throw new Error(`Cannot remove non-style children from document`);
    }
};

document.dispatchEvent = (event: any) => {
    //Svelte dev fires these for tool support
};
