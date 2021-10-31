import { Node, INode } from "./nodes";

export const isWidget = (cls: any): cls is Widget & INode =>
    cls instanceof Widget && cls?.$?.isGtkWidget;

export interface IGtkWidget {
    isGObject?: boolean;
    isGtkWidget?: boolean;
    addClass(className: string): void;
    removeClass(className: string): void;
    setAttribute(name: string, value: any): void;
    getAttribute(name: string): any;
    removeAttribute(name: string): void;
    addEventListener(type: string, listener: (event: any) => void): void;
    removeEventListener(type: string, listener: (event: any) => void): void;
}

export interface WidgetConstructor<T = {}> {
    new (...args: Array<any>): T & IGtkWidget;
}

export interface IWidget {
    mounted?: boolean;

    nextSibling: any;
    prevSibling: any;
    parentNode: any;
    children: any;
}

export class Widget {
    classList: {
        add(name: string): void;
        remove(name: string): void;
    };

    __parent__: INode | null;
    __prev__: INode | null;
    __next__: INode | null;

    __children__: Array<INode>;

    __callbacks__: {
        onInsertBefore?: (node: INode, anchor?: INode) => void;
        onAppendChild?: (node: INode) => void;
        onRemoveChild?: (node: INode) => void;
    };

    get children() {
        return this.__children__;
    }

    get parentNode() {
        return this.__parent__;
    }

    set parentNode(value: INode | null) {
        this.__parent__ = value;
    }

    get nextSibling() {
        return this.__next__;
    }

    set nextSibling(value: INode | null) {
        this.__next__ = value;
    }

    get prevSibling() {
        return this.__prev__;
    }

    set prevSibling(value: INode | null) {
        this.__prev__ = value;
    }

    constructor(public $: any) {
        this.__parent__ = null;
        this.__prev__ = null;
        this.__next__ = null;
        this.__children__ = new Array();
        this.__callbacks__ = {};

        this.classList = {
            add: (name: string) => this.$.addClass(name),
            remove: (name: string) => this.$.removeClass(name),
        };
    }

    setAttribute = (name: string, value: any) => {
        if (name === "class") {
            console.log("setting classes: ", value);
            this.$.setAttribute("cssClasses", value.split(" "));
        } else {
            this.$.setAttribute(name, value);
        }
    };

    getAttribute = (name: string): any => {
        if (name === "class") {
            return this.$.getAttribute("cssClasses").join(" ");
        } else {
            return this.$.getAttribute(name);
        }
    };

    removeAttribute = (name: string) => {
        this.$.removeAttribute(name);
    };

    addEventListener = (name: string, callback: (event: any) => void) => {
        this.$.addEventListener(name, callback);
    };

    removeEventListener = (name: string, callback: (event: any) => void) => {
        this.$.removeEventListener(name, callback);
    };
}

export const getNextWidget = (node: INode): INode | null => {
    let next = node.nextSibling;
    while (next) {
        if (isWidget(next) && next.mounted) {
            break;
        }
        next = next.nextSibling;
    }
    return isWidget(next) ? next : null;
};

export const getPrevWidget = (node: INode): INode | null => {
    let prev = node.prevSibling;
    while (prev) {
        if (isWidget(prev) && prev.mounted) {
            break;
        }
        prev = prev.prevSibling;
    }
    return isWidget(prev) ? prev : null;
};
