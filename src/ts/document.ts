import { loadBinding } from "@node-rs/helper";
import { SvelteQt } from "./types";

const { QMainWindow, QPushButton, QWidget } = loadBinding(
    __dirname,
    "svelte-qt",
    "svelte-qt",
) as SvelteQt;

export default class SvelteQtDocument {
    document: this = this;
    constructor() {
        console.log(`created ${this}`);
    }

    createEvent(type: string) {
        console.log(`Creating event with type: "${type}"`);
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
    }

    createComment(data: string): { data: string } {
        console.log(`Creating comment with data: "${data}"`);
        return { data };
    }

    // createPropertyNode(tagName: string, propertyName: string): PropertyNode {
    //     console.log(`[SvelteNodeGUIDocument] createPropertyNode("${tagName}", "${propertyName}")`);
    //     return new PropertyNode(tagName, propertyName)
    // }

    createElement(tagName: string): any {
        // if (tagName.indexOf(".") >= 0) {
        //     let bits = tagName.split(".", 2);
        //     return this.createPropertyNode(bits[0], bits[1]);
        // }
        // return nodeOps.createElement(tagName) as NSVElement;

        let ele: any;
        switch (tagName) {
            case "qMainWindow":
                ele = new QMainWindow();
                break;
            case "qPushButton":
            default: {
                ele = new QPushButton();
            }
        }
        if (tagName !== "document") {
            ele.ownerDocument = this;
        }
        return ele;
    }

    createElementNS(namespace: string, tagName: string): any {
        return this.createElement(tagName);
    }

    createTextNode(data: string): { data: string } {
        console.log(`[Document] createTextNode("${data}")`);
        return { data };
    }

    getElementById(id: string): any | null {
        // for (let el of elementIterator(this)) {
        //     // console.log(`getElementById iterating over element <${(el as NSVElement).tagName ?? "node"}> ${(el as NSVElement).id}`);
        //     if (
        //         el.nodeType === NSVNodeTypes.ELEMENT &&
        //         (el as NSVElement).id === id
        //     ) {
        //         return el as NSVElement;
        //     }
        // }
        return null;
    }

    dispatchEvent(event: any) {
        //Svelte dev fires these for tool support
    }
}
