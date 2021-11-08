/// <reference path="./domtypes.d.ts" />
import * as nodes from "./nodes";
import { IWidget } from "./nodes";
import { isStyle } from "./utils";

import { ELEMENTS } from "./widgets";

document.root = new nodes.Root();

document.createElement = (val: string): any => {
    let ele: any = null;
    const parts = val.split(":", 2);
    if (parts.length > 1) {
        const tagName = parts[1];
        const func = ELEMENTS[tagName];
        if (func) {
            return nodes.Child(parts[0], func());
        } else {
            throw new Error(`No tag name "${tagName}" defined`);
        }
    }
    const tagName = val;
    //$console.log(`[Document] createElement("${tagName}")`);
    switch (tagName) {
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
    const el = document.createElement(tagName) as unknown as IWidget;
    if (namespace.startsWith("https://")) {
        return el;
    }
    return nodes.Child(namespace, el);
};

document.getRootNode = () => {
    return document;
};

document.createEvent = (type: string) => {
    // console.log(`[Document] createEvent("${type}")`);
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
