import * as nodes from "./nodes";
import { BoxWidget, Widget, Class } from "./nodes";

const ELEMENTS: { [name: string]: () => any } = {
    button: () => new Widget(new Button()),
    hBox: () => new BoxWidget(new HorizontalBox()),
    vBox: () => new BoxWidget(new VerticalBox()),
};

document.root = new nodes.Element();

document.createEvent = (type: string) => {
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
};

document.createComment = (data: string): nodes.Comment => {
    console.log(`Creating comment with data: "${data}"`);
    return new nodes.Comment(data);
};

document.createElement = (tagName: string): any => {
    let ele: nodes.Node;
    console.log(`Creating ${tagName}`);
    switch (tagName) {
        case "appWindow":
            ele = new nodes.Window(new ApplicationWindow());
            break;
        case "gtkWindow":
            ele = new nodes.Window(new Window());
            break;
        case "child":
            ele = new nodes.Child();
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

document.getElementById = (id: string): any | null => {
    return null;
};

document.dispatchEvent = (event: any) => {
    //Svelte dev fires these for tool support
};
