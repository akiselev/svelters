/// <reference path="./domtypes.d.ts" />

import { listen, time_ranges_to_array } from "svelte/internal";

export type Node = Widget | Child | Element | Text | Comment;
export interface Parent extends NodeBase {}

export interface IAttributes {
    setAttribute(name: string, value: any): void;
    getAttribute(name: string): any;
    removeAttribute(name: string): void;
}

export interface IEvents {
    addEventListener(type: string, listener: (event: any) => void): void;
    removeEventListener(type: string, listener: (event: any) => void): void;
}

export interface IChild {
    parentNode: Parent | null;
    prevSibling: Node | null;
    nextSibling: Node | null;
    isGtkWidget?: boolean;
    isGObject?: boolean;

    onSetParent: (parent: Parent | null) => void;
}

export interface IParent {
    insertBefore(node: Node, anchor?: Node): void;
    appendChild(node: Node): void;
    removeChild(node: Node): void;
}

export class NodeBase implements IParent, IChild {
    protected __parent__: Parent | null;
    protected __prev__: Node | null;
    protected __next__: Node | null;

    protected __children__: Array<Node>;

    mounted: boolean;

    constructor() {
        this.__parent__ = null;
        this.__prev__ = null;
        this.__next__ = null;
        this.__children__ = new Array();

        this.mounted = false;
    }

    get children() {
        return this.__children__;
    }

    get parentNode() {
        return this.__parent__;
    }

    set parentNode(value: Parent | null) {
        this.__parent__ = value;
        this.onSetParent(value);
    }

    get nextSibling() {
        return this.__next__;
    }

    set nextSibling(value: Node | null) {
        this.__next__ = value;
    }

    get prevSibling() {
        return this.__prev__;
    }

    set prevSibling(value: Node | null) {
        this.__prev__ = value;
    }

    get isGtkWidget() {
        return false;
    }

    get isGObject() {
        return false;
    }

    onSetParent = (_: Parent | null) => {};

    insertBefore = getInsertBefore.bind(this);
    appendChild = getAppendChild.bind(this);
    removeChild = getRemoveChild.bind(this);
}

export class Element extends NodeBase implements IAttributes, IEvents, IChild {
    private __attr__: { [name: string]: any };
    private __listeners__: { [name: string]: any[] };

    constructor() {
        super();
        this.__attr__ = {};
        this.__listeners__ = {};
    }

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

export class Child extends Element {
    onInsertBefore: ((node: Node, anchor?: Node) => void) | null;
    onAppendChild: ((node: Node) => void) | null;
    onRemoveChild: ((node: Node) => void) | null;

    baseInsertBefore: (node: Node, anchor?: Node) => void;
    baseAppendChild: (node: Node) => void;
    baseRemoveChild: (node: Node) => void;

    constructor() {
        super();
        this.onInsertBefore = null;
        this.onAppendChild = null;
        this.onRemoveChild = null;
        this.baseInsertBefore = getInsertBefore.bind(this);
        this.baseAppendChild = getAppendChild.bind(this);
        this.baseRemoveChild = getRemoveChild.bind(this);
    }

    clearHooks = () => {
        this.onInsertBefore = null;
        this.onAppendChild = null;
        this.onRemoveChild = null;
    };

    get name() {
        return this.getAttribute("name");
    }

    set name(value: string) {
        this.setAttribute("name", value);
    }

    insertBefore = (node: Node, anchor?: Node) => {
        this.baseInsertBefore(node, anchor);
        this.onInsertBefore?.(node, anchor);
    };

    appendChild = (node: Node) => {
        this.baseAppendChild(node);
        this.onAppendChild?.(node);
    };

    removeChild = (node: Node) => {
        this.baseRemoveChild(node);
        this.onRemoveChild?.(node);
    };

    get firstWidget(): Node | null {
        for (var i = 0; i < this.children.length; i++) {
            if (isWidget(this.children[i])) {
                return this.children[i];
            }
        }
        return null;
    }

    get firstClass(): Node | null {
        for (var i = 0; i < this.children.length; i++) {
            if (isClass(this.children[i])) {
                return this.children[i];
            }
        }
        return null;
    }
}

export class Text extends NodeBase {
    constructor(public text: string) {
        super();
    }
}

export class Comment extends NodeBase {
    constructor(public comment: string) {
        super();
    }
}

export class Class extends NodeBase implements IAttributes, IEvents {
    constructor(public instance: any) {
        super();
    }

    get isGtkWidget() {
        return this.instance.isGtkWidget;
    }

    get isGObject() {
        return this.instance.isGObject;
    }

    setAttribute = (name: string, value: any) => {
        this.instance.setAttribute(name, value);
    };

    getAttribute = (name: string) => {
        return this.instance.getAttribute(name);
    };

    removeAttribute = (name: string) => {
        this.instance.removeAttribute(name);
    };

    addEventListener = (type: string, listener: (event: any) => void): void => {
        this.instance.addEventListener(type, listener);
    };

    removeEventListener = (
        type: string,
        listener: (event: any) => void,
    ): void => {
        this.instance.removeEventListener(type, listener);
    };
}

export class Widget extends Class {
    constructor(instance: any) {
        super(instance);
    }
}

export class Window extends Widget {
    baseInsertBefore: (node: Node, anchor?: Node) => void;
    baseAppendChild: (node: Node) => void;
    baseRemoveChild: (node: Node) => void;

    constructor(instance: any) {
        super(instance);
        this.baseInsertBefore = getInsertBefore.bind(this);
        this.baseAppendChild = getAppendChild.bind(this);
        this.baseRemoveChild = getRemoveChild.bind(this);
    }

    handleChild = (node: Node) => {
        if (isWidget(node)) {
            if (this.instance.child) {
                throw new Error("Window accepts only one child");
            }
            this.instance.child = $(node);
        } else if (isChild(node) && node.name === "titlebar") {
            const inner = node.firstWidget;
            if (this.instance.titlebar) {
                throw new Error("Only one titlebar allowed");
            }
            if (inner) {
                this.instance.titlebar = $(inner);
            }
            node.onInsertBefore = this.onInsertTitlebar;
            node.onAppendChild = this.onAppendTitlebar;
            node.onRemoveChild = this.onRemoveTitlebar;
        }
    };

    onInsertTitlebar = (node: Node, anchor?: Node) => {
        if (isWidget(node)) {
            if (this.instance.titlebar) {
                throw new Error("Only one titlebar allowed");
            }
            this.instance.titlebar = $(node);
        }
    };

    onAppendTitlebar = (node: Node) => {
        if (isWidget(node)) {
            if (this.instance.titlebar) {
                throw new Error("Only one titlebar allowed");
            }
            this.instance.titlebar = $(node);
        }
    };

    onRemoveTitlebar = (node: Node) => {
        if (isWidget(node)) {
            const inst = $(node);
            if (inst === this.instance.titlebar) {
                this.instance.titlebar = null;
            } else {
                throw new Error("Titlebar widgets don't match");
            }
        }
    };

    insertBefore = (node: Node, anchor?: Node) => {
        this.baseInsertBefore(node, anchor);
        this.handleChild(node);
    };

    appendChild = (node: Node) => {
        this.baseAppendChild(node);
        this.handleChild(node);
    };

    removeChild = (node: Node) => {
        this.baseRemoveChild(node);
        if (isWidget(node)) {
            this.instance.child = null;
        } else if (isChild(node) && node.name === "titlebar") {
            this.instance.titlebar = null;
            node.clearHooks();
        }
    };
}

export class BoxWidget extends Widget {
    baseInsertBefore: (node: Node, anchor?: Node) => void;
    baseAppendChild: (node: Node) => void;
    baseRemoveChild: (node: Node) => void;

    constructor(instance: any) {
        super(instance);
        this.baseInsertBefore = getInsertBefore.bind(this);
        this.baseAppendChild = getAppendChild.bind(this);
        this.baseRemoveChild = getRemoveChild.bind(this);
    }

    insertBefore = (node: Node, anchor?: Node) => {
        this.baseInsertBefore(node, anchor);
        if (isWidget(node)) {
            console.log("Insert before...");
            let target = getPrevWidget(node);
            if (isWidget(target)) {
                console.log(
                    "Insert after target: ",
                    node.getAttribute("name"),
                    target.getAttribute("name"),
                    target instanceof Text,
                    target instanceof Widget,
                );
                $(this).insertChildAfter($(node), $(target));
            } else {
                console.log("Prepending...", node.getAttribute("name"));
                $(this).prepend($(node));
            }
        }
    };

    appendChild = (node: Node) => {
        this.baseAppendChild(node);
        if (isWidget(node)) {
            console.log("Appending...", node.getAttribute("name"));
            $(this).append($(node));
        }
    };

    removeChild = (node: Node) => {
        this.baseRemoveChild(node);
        if (isWidget(node)) {
            console.log("Removing child...", node.getAttribute("name"));
            $(this).remove($(node));
        }
    };
}

const isClass = (cls: any): cls is Class => cls && cls instanceof Class;
const isWidget = (cls: any): cls is Widget =>
    cls && cls instanceof Widget && cls.instance.isGtkWidget;
const isChild = (cls: any): cls is Child => cls && cls instanceof Child;
const isText = (cls: any): cls is Text => cls && cls instanceof Text;

const $ = <T>(cls: T): T extends Class ? any : void => {
    return cls.instance;
};

function getInsertBefore(this: NodeBase, node: Node, anchor?: Node): void {
    if (anchor) {
        const index = this.__children__.indexOf(anchor);
        if (index === -1) {
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
            node.prevSibling = this.__children__[this.__children__.length - 1];
        }
        node.parentNode = this;
        this.__children__.push(node);
    }
    node.mounted = true;
    console.log(">>>>>>>>>>>>.", this.__children__.length);
}

function getAppendChild(this: NodeBase, node: Node): void {
    if (this.__children__.length > 0) {
        node.prevSibling = this.__children__[this.__children__.length - 1];
    }
    node.parentNode = this;
    node.mounted = true;
    this.__children__.push(node);
    console.log(">>>>>>>>>>>>.", this.__children__.length);
}

function getRemoveChild(this: NodeBase, node: Node): void {
    const index = this.__children__.indexOf(node);
    if (index === -1) {
        console.log("Node is not in children");
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
    console.log(">>>>>>>>>>>>.....", this.__children__.length);
}

const getNextWidget = (node: NodeBase): Node | null => {
    let next = node.nextSibling;
    while (next && !isWidget(next)) {
        next = next.nextSibling;
    }
    return isWidget(next) ? next : null;
};

const getPrevWidget = (node: NodeBase): Node | null => {
    let prev = node.prevSibling;
    while (prev) {
        if (isWidget(prev) && prev.mounted) {
            break;
        }
        prev = prev.prevSibling;
    }
    return isWidget(prev) ? prev : null;
};
