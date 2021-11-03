/// <reference path="./domtypes.d.ts" />

import { isWidget } from "./utils";

export interface INode {
    mounted: boolean;
    parentNode: INode | null;
    prevSibling: INode | null;
    nextSibling: INode | null;

    getAttribute(name: string): any;
    removeAttribute(name: string): void;
    setAttribute(name: string, value: any): void;
    addEventListener(type: string, listener: (event: any) => void): void;
    removeEventListener(type: string, listener: (event: any) => void): void;
}

export interface IWidget extends INode {
    readonly tagName: string;
    get isGObject(): boolean;
    get isGtkWidget(): boolean;

    get $(): any;
}

export interface IContainer extends INode {
    appendChild(node: INode): void;
    removeChild(node: INode): void;
    insertBefore(node: INode, anchor?: INode): void;
}

export interface IChild extends IWidget {
    __name__: string;
}

export class Node implements INode {
    private __attr__: { [name: string]: any };
    private __listeners__: { [name: string]: any[] };
    protected __parent__: INode | null;
    protected __prev__: INode | null;
    protected __next__: INode | null;
    protected __mounted__: boolean;

    constructor() {
        this.__parent__ = null;
        this.__prev__ = null;
        this.__next__ = null;
        this.__attr__ = {};
        this.__listeners__ = {};

        this.__mounted__ = false;
    }

    get mounted() {
        return this.__mounted__;
    }

    set mounted(value: boolean) {
        this.__mounted__ = value;
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

    get isGtkWidget() {
        return false;
    }

    get isGObject() {
        return false;
    }

    getRootNode = () => {
        return document;
    };

    setAttribute = (name: string, value: any) => {
        this.__attr__[name] = value;
    };

    getAttribute = (name: string) => {
        return this.__attr__[name];
    };

    removeAttribute = (name: string) => {
        delete this.__attr__[name];
    };

    addEventListener = (type: string, listener: (event: any) => void): void => {
        if (typeof this.__listeners__[type] == "undefined") {
            this.__listeners__[type] = new Array();
        }
        this.__listeners__[type].push(listener);
    };

    removeEventListener = (
        type: string,
        listener: (event: any) => void,
    ): void => {
        const list = this.__listeners__[type];
        if (typeof list != "undefined") {
            this.__listeners__[type] = list.filter(
                (handler: any) => handler != listener,
            );
        }
    };

    dispatchEvent = (type: string, event: any) => {
        const listeners = this.__listeners__[type];
        if (Array.isArray(listeners)) {
            listeners.forEach((handler) => handler(event));
        }
    };
}

export class Root extends Node {
    __children__: INode[];
    constructor() {
        super();
        this.__children__ = [];
    }

    insertBefore = (node: INode, anchor?: INode): void => {
        if (anchor) {
            const index = this.__children__.indexOf(anchor);
            if (index === -1) {
                console.log(
                    ">>>>>>>>>++++++++++++++++>>>>>>>>>>>>>>>>>>>>>>>++++++++++++++++++",
                );
                throw new Error("Anchor is not in children");
                // this.appendChild(node);
            } else {
                node.prevSibling = anchor.prevSibling;
                node.nextSibling = anchor;
                anchor.prevSibling = node;
                node.parentNode = this;
                this.__children__.splice(index, 0, node);
            }
        } else {
            if (this.__children__.length > 0) {
                node.prevSibling =
                    this.__children__[this.__children__.length - 1];
            }
            node.parentNode = this;
            this.__children__.push(node);
        }
        node.mounted = true;
    };

    appendChild = (node: INode): void => {
        if (this.__children__.length > 0) {
            node.prevSibling = this.__children__[this.__children__.length - 1];
        }
        node.parentNode = this;
        node.mounted = true;
        this.__children__.push(node);
    };

    removeChild = (node: INode): void => {
        const index = this.__children__.indexOf(node);
        if (index === -1) {
        } else {
            const prevSibling = node.prevSibling;
            const nextSibling = node.nextSibling;
            if (prevSibling) {
                prevSibling.nextSibling = nextSibling;
            }
            if (nextSibling) {
                nextSibling.prevSibling = prevSibling;
            }
            node.nextSibling = null;
            node.prevSibling = null;
            node.parentNode = null;
            this.__children__ = this.__children__.filter(
                (target) => target !== node,
            );
        }
        node.mounted = false;
    };
}

export class Style extends Node {
    id: string | null;
    __text__: string | null;
    provider: any;

    constructor() {
        super();
        this.provider = new CssProvider();
        this.id = null;
        this.__text__ = null;
    }

    get textContent() {
        return this.__text__;
    }

    set textContent(value: string | null) {
        this.__text__ = value;
        this.provider.setCss(value || "");
    }
}

export class Text extends Node {
    constructor(public text: string) {
        super();
    }
}

export class Comment extends Node {
    constructor(public comment: string) {
        super();
    }
}

export const Child = (name: string, widget: IWidget): IChild => {
    return {
        ...widget,
        __name__: name,
    };
};
