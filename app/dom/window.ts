import { Node, INode, isChild } from "./nodes";
import { isWidget, Widget } from "./widget";

export class Window extends Widget {
    constructor($: any) {
        super($);
    }

    handleChild = (node: INode) => {
        if (isWidget(node)) {
            if (this.$.child) {
                throw new Error("Window accepts only one child");
            }
            this.$.child = node.$;
        }
        // else if (isChild(node) && node.name === "titlebar") {
        //     const inner = getFirstWidget(node);
        //     if (this.$.titlebar) {
        //         throw new Error("Only one titlebar allowed");
        //     }
        //     if (inner) {
        //         this.$.titlebar = inner;
        //     }
        //     node.callbacks = {
        //         onInsertBefore: this.onInsertTitlebar,
        //         onAppendChild: this.onAppendTitlebar,
        //         onRemoveChild: this.onRemoveTitlebar,
        //     };
        // }
    };

    // onInsertTitlebar = (node: INode, anchor?: INode) => {
    //     if (isWidget(node)) {
    //         if (this.$.titlebar) {
    //             throw new Error("Only one titlebar allowed");
    //         }
    //         this.$.titlebar = $(node);
    //     }
    // };

    // onAppendTitlebar = (node: INode) => {
    //     if (isWidget(node)) {
    //         if (this.$.titlebar) {
    //             throw new Error("Only one titlebar allowed");
    //         }
    //         this.$.titlebar = $(node);
    //     }
    // };

    // onRemoveTitlebar = (node: INode) => {
    //     if (isWidget(node)) {
    //         const inst = $(node);
    //         if (inst === this.$.titlebar) {
    //             this.$.titlebar = null;
    //         } else {
    //             throw new Error("Titlebar widgets don't match");
    //         }
    //     }
    // };

    insertBefore = (node: INode, anchor?: INode) => {
        console.log(
            ">>>>>>>>>window++++++++++>>>>>>>>>>>>>>>> insert before ",
            node.getAttribute?.("id"),
        );
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
                node.prevSibling =
                    this.__children__[this.__children__.length - 1];
            }
            node.parentNode = this;
            this.__children__.push(node);
        }
        node.mounted = true;
        console.log(">>>>>>>>>>>>.", this.__children__.length);
        this.handleChild(node);
    };

    appendChild = (node: INode) => {
        console.log(
            ">>>>>>>>>window+++++++++>>>>>>>>>>>>>>>> append ",
            node.getAttribute?.("id"),
        );
        if (this.__children__.length > 0) {
            node.prevSibling = this.__children__[this.__children__.length - 1];
        }
        node.parentNode = this;
        node.mounted = true;
        this.__children__.push(node);
        console.log(">>>>>>>>>>>>.", this.__children__.length);
        this.handleChild(node);
    };

    removeChild = (node: INode) => {
        console.log(
            ">>>>>>>>>window++++++++>>>>>>>>>>>>>>>> insert before ",
            node.getAttribute?.("id"),
        );
        const index = this.__children__.indexOf(node);
        if (index === -1) {
            console.log(
                "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! INode is not in children",
            );
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
        console.log(">>>>>>>>>>>>.", this.__children__.length);
        if (isWidget(node)) {
            this.$.child = null;
        } else if (isChild(node) && node.name === "titlebar") {
            this.$.titlebar = null;
            node.clearHooks();
        }
    };

    getRootNode = () => {
        console.log("get roo node: ", JSON.stringify(document, null, 2));
        return document;
    };
}
