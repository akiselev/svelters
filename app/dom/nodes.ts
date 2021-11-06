/// <reference path="./domtypes.d.ts" />

import { isWidget, unmountNode } from "./utils";

export type Constructor<T> = new (...args: any[]) => T;

export interface INode {
    __mounted__: boolean;
    __parent__: INode | null;
    __prev__: INode | null;
    __next__: INode | null;

    getAttribute(name: string): any;
    removeAttribute(name: string): void;
    setAttribute(name: string, value: any): void;
    addEventListener(type: string, listener: (event: any) => void): void;
    removeEventListener(type: string, listener: (event: any) => void): void;

    onUnmount?(): void;
}

export interface IWidget extends INode {
    readonly tagName: string;
    get isGObject(): boolean;
    get isGtkWidget(): boolean;

    appendChild(node: INode): void;
    removeChild(node: INode): void;
    insertBefore(node: INode, anchor?: INode): void;
    onUnmount(): void;
}

export interface IChild extends IWidget {
    __name__: string;
}

export class Node implements INode {
    __state__ = {
        attr: {} as { [name: string]: any },
        listeners: {} as { [name: string]: any[] },
        parentNode: null as INode | null,
        prevSibling: null as INode | null,
        nextSibling: null as INode | null,
        mounted: false,
    };

    constructor() {}

    get __mounted__() {
        return this.__state__.mounted;
    }

    set __mounted__(value: boolean) {
        this.__state__.mounted = value;
    }

    get __parent__() {
        return this.__state__.parentNode;
    }

    set __parent__(value: INode | null) {
        this.__state__.parentNode = value;
    }

    get __next__() {
        return this.__state__.nextSibling;
    }

    set __next__(value: INode | null) {
        this.__state__.nextSibling = value;
    }

    get __prev__() {
        return this.__state__.prevSibling;
    }

    set __prev__(value: INode | null) {
        this.__state__.prevSibling = value;
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
        this.__state__.attr[name] = value;
    };

    getAttribute = (name: string) => {
        return this.__state__.attr[name];
    };

    removeAttribute = (name: string) => {
        delete this.__state__.attr[name];
    };

    addEventListener = (type: string, listener: (event: any) => void): void => {
        if (typeof this.__state__.listeners[type] == "undefined") {
            this.__state__.listeners[type] = new Array();
        }
        this.__state__.listeners[type].push(listener);
    };

    removeEventListener = (
        type: string,
        listener: (event: any) => void,
    ): void => {
        const list = this.__state__.listeners[type];
        if (typeof list != "undefined") {
            this.__state__.listeners[type] = list.filter(
                (handler: any) => handler != listener,
            );
        }
    };

    dispatchEvent = (type: string, event: any) => {
        const listeners = this.__state__.listeners[type];
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
                node.__prev__ = anchor.__prev__;
                node.__next__ = anchor;
                anchor.__prev__ = node;
                node.__parent__ = this;
                this.__children__.splice(index, 0, node);
            }
        } else {
            if (this.__children__.length > 0) {
                node.__prev__ = this.__children__[this.__children__.length - 1];
            }
            node.__parent__ = this;
            this.__children__.push(node);
        }
        node.__mounted__ = true;
    };

    appendChild = (node: INode): void => {
        if (this.__children__.length > 0) {
            node.__prev__ = this.__children__[this.__children__.length - 1];
        }
        node.__parent__ = this;
        node.__mounted__ = true;
        this.__children__.push(node);
    };

    removeChild = (node: INode): void => {
        const index = this.__children__.indexOf(node);
        if (index === -1) {
        } else {
            const prevSibling = node.__prev__;
            const nextSibling = node.__next__;
            if (prevSibling) {
                prevSibling.__next__ = nextSibling;
            }
            if (nextSibling) {
                nextSibling.__prev__ = prevSibling;
            }
            node.__next__ = null;
            node.__prev__ = null;
            node.__parent__ = null;
            this.__children__ = this.__children__.filter(
                (target) => target !== node,
            );
        }
        node.__mounted__ = false;
        node?.onUnmount?.();
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
