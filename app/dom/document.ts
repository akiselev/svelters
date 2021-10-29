const ELEMENTS = {
    button: Button,
    hBox: HorizontalBox,
    vBox: VerticalBox,
};

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

document.createComment = (data: string): { data: string } => {
    console.log(`Creating comment with data: "${data}"`);
    return { data };
};

// createPropertyNode(tagName: string, propertyName: string): PropertyNode {
//     console.log(`[SvelteNodeGUIDocument] createPropertyNode("${tagName}", "${propertyName}")`);
//     return new PropertyNode(tagName, propertyName)
// }

document.createElement = (tagName: string): any => {
    let ele: any;
    switch (tagName) {
        case "button":
            ele = new Button();
            break;
        case "hBox":
            ele = new HorizontalBox();
            break;
        case "window":
            ele = new Window();
            break;
        case "vBox":
        default: {
            ele = new VerticalBox();
        }
    }
    return ele;
};

document.createElementNS = (namespace: string, tagName: string): any => {
    console.log(`Creating ${tagName}  in ${namespace}`);
    return document.createElement(tagName);
};

document.createTextNode = (data: string): string => {
    console.log(`[Document] createTextNode("${data}")`);
    // return { data };
    return { __isTextNode__: true, text: data };
};

document.getElementById = (id: string): any | null => {
    return null;
};

document.dispatchEvent = (event: any) => {
    //Svelte dev fires these for tool support
};
