/// <reference path="./domtypes.d.ts" />

export const isNode = (cls: any): cls is Node => cls && cls instanceof Node;
export const isText = (cls: any): cls is Text => cls && cls instanceof Text;
export const isChild = (cls: any): cls is Child => cls && cls instanceof Child;
export const isComment = (cls: any): cls is Comment =>
    cls && cls instanceof Comment;
export const isStyle = (cls: any): cls is Style => cls && cls instanceof Style;

export interface INode {
    mounted?: boolean;
    isGObject?: boolean;
    isGtkWidget?: boolean;
    parentNode: any | null;
    prevSibling: INode | null;
    nextSibling: INode | null;
    appendChild(node: INode): void;
    removeChild(node: INode): void;
    insertBefore(node: INode, anchor?: INode): void;

    getAttribute(name: string): any;
    removeAttribute(name: string): void;
    setAttribute(name: string, value: any): void;
    addEventListener(type: string, listener: (event: any) => void): void;
    removeEventListener(type: string, listener: (event: any) => void): void;
}

export class Node implements INode {
    private __attr__: { [name: string]: any };
    private __listeners__: { [name: string]: any[] };
    protected __parent__: INode | null;
    protected __prev__: INode | null;
    protected __next__: INode | null;

    protected __children__: Array<INode>;

    protected __callbacks__: {
        onInsertBefore?: (node: INode, anchor?: INode) => void;
        onAppendChild?: (node: INode) => void;
        onRemoveChild?: (node: INode) => void;
    };

    mounted: boolean;

    constructor() {
        this.__parent__ = null;
        this.__prev__ = null;
        this.__next__ = null;
        this.__children__ = new Array();
        this.__attr__ = {};
        this.__listeners__ = {};
        this.__callbacks__ = {};

        this.mounted = false;
    }

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

    get isGtkWidget() {
        return false;
    }

    get isGObject() {
        return false;
    }

    getRootNode = () => {
        console.log("get roo node: ", JSON.stringify(document, null, 2));
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

    clearHooks = () => {
        this.__callbacks__ = {};
    };

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
        this.__callbacks__.onInsertBefore?.(node, anchor);
    };

    appendChild = (node: INode): void => {
        if (this.__children__.length > 0) {
            node.prevSibling = this.__children__[this.__children__.length - 1];
        }
        node.parentNode = this;
        node.mounted = true;
        this.__children__.push(node);
        this.__callbacks__.onAppendChild?.(node);
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
        this.__callbacks__.onRemoveChild?.(node);
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
        this.provider.setCss(value ? value.replace(/\}/g, ";}") : "");
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

export class Child extends Node {
    constructor() {
        super();
    }

    get name() {
        return this.getAttribute("id");
    }

    set name(value: string) {
        this.setAttribute("id", value);
    }

    getRootNode = () => {
        return document;
    };
}
