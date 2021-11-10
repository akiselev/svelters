/// <reference path="./domtypes.d.ts" />

export interface INode {
    svMounted: boolean;
    parentNode: INode | null;
    svPrevSibling: INode | null;
    svNextSibling: INode | null;

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
    get extra(): { [name: string]: any };
    svName: string;

    appendChild(node: INode): void;
    removeChild(node: INode): void;
    insertBefore(node: INode, anchor?: INode): void;
    onUnmount(): void;
}

export interface IChild extends IWidget {
    __name__: string;
}

export class Node implements INode {
    extra: {
        [name: string]: any;
    };

    constructor() {
        this.extra = {
            parent: null,
            prev: null,
            next: null,
            attr: {},
            listeners: {},
            svMounted: false,
        };
    }

    get svMounted() {
        return this.extra.svMounted;
    }

    set svMounted(value: boolean) {
        this.extra.svMounted = value;
    }

    get parentNode() {
        return this.extra.parent;
    }

    set parentNode(value: INode | null) {
        this.extra.parent = value;
    }

    get svNextSibling() {
        return this.extra.next;
    }

    set svNextSibling(value: INode | null) {
        this.extra.next = value;
    }

    get svPrevSibling() {
        return this.extra.prev;
    }

    set svPrevSibling(value: INode | null) {
        this.extra.prev = value;
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
        this.extra.attr[name] = value;
    };

    getAttribute = (name: string) => {
        return this.extra.attr[name];
    };

    removeAttribute = (name: string) => {
        delete this.extra.attr[name];
    };

    addEventListener = (type: string, listener: (event: any) => void): void => {
        if (typeof this.extra.listeners[type] == "undefined") {
            this.extra.listeners[type] = new Array();
        }
        this.extra.listeners[type].push(listener);
    };

    removeEventListener = (
        type: string,
        listener: (event: any) => void,
    ): void => {
        const list = this.extra.listeners[type];
        if (typeof list != "undefined") {
            this.extra.listeners[type] = list.filter(
                (handler: any) => handler != listener,
            );
        }
    };

    dispatchEvent = (type: string, event: any) => {
        const listeners = this.extra.listeners[type];
        if (Array.isArray(listeners)) {
            listeners.forEach((handler) => handler(event));
        }
    };
}

export class Root extends Node {
    constructor() {
        super();
        this.extra.children = [] as INode[];
    }

    insertBefore = (node: INode, anchor?: INode): void => {
        if (anchor) {
            const index = this.extra.children.indexOf(anchor);
            if (index === -1) {
                console.log(
                    ">>>>>>>>>++++++++++++++++>>>>>>>>>>>>>>>>>>>>>>>++++++++++++++++++",
                );
                throw new Error("Anchor is not in children");
                // this.appendChild(node);
            } else {
                node.svPrevSibling = anchor.svPrevSibling;
                node.svNextSibling = anchor;
                anchor.svPrevSibling = node;
                node.parentNode = this;
                this.extra.children.splice(index, 0, node);
            }
        } else {
            if (this.extra.children.length > 0) {
                node.svPrevSibling =
                    this.extra.children[this.extra.children.length - 1];
            }
            node.parentNode = this;
            this.extra.children.push(node);
        }
        node.svMounted = true;
    };

    appendChild = (node: INode): void => {
        if (this.extra.children.length > 0) {
            node.svPrevSibling =
                this.extra.children[this.extra.children.length - 1];
        }
        node.parentNode = this;
        node.svMounted = true;
        this.extra.children.push(node);
    };

    removeChild = (node: INode): void => {
        const index = this.extra.children.indexOf(node);
        if (index === -1) {
        } else {
            const prev = node.svPrevSibling;
            const next = node.svNextSibling;
            if (prev) {
                prev.svNextSibling = next;
            }
            if (next) {
                next.svPrevSibling = prev;
            }
            node.svNextSibling = null;
            node.svPrevSibling = null;
            node.parentNode = null;
            this.extra.children = this.extra.children.filter(
                (target: INode) => target !== node,
            );
        }
        node.svMounted = false;
        node?.onUnmount?.();
    };
}

export class Style extends Node {
    id: string | null;
    provider: any;

    constructor() {
        super();
        this.provider = new CssProvider();
        this.id = null;
        this.extra.text = null;
    }

    get textContent() {
        return this.extra.text;
    }

    set textContent(value: string | null) {
        this.extra.text = value;
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
