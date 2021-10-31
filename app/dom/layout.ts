import { Node, INode, Text } from "./nodes";
import {
    WidgetConstructor,
    Widget,
    isWidget,
    getPrevWidget,
    getNextWidget,
} from "./widget";

interface IBox {
    insertChildAfter(child: any, anchor: any): void;
    append(child: any): void;
    prepend(child: any): void;
    remove(child: any): void;
}

export class BoxWidget extends Widget {
    tail: INode | null = null;

    constructor(public $: any) {
        super($);
    }

    insertBefore = (node: INode, anchor?: INode) => {
        node.parentNode = this;
        if (anchor) {
            node.prevSibling = anchor.prevSibling;
            node.nextSibling = anchor;
            anchor.prevSibling = node;
        } else {
            if (this.tail) {
                node.prevSibling = this.tail;
                this.tail.nextSibling = node;
            }
            this.tail = node;
        }
        node.mounted = true;

        console.log(
            "Insert before...",
            node.getAttribute("id"),
            anchor?.getAttribute?.("id"),
            anchor?.prevSibling?.getAttribute?.("id"),
            anchor?.prevSibling?.prevSibling?.getAttribute?.("id"),
        );
        if (isWidget(node)) {
            let target = getNextWidget(node);
            if (isWidget(target)) {
                console.log(
                    "Insert after target: ",
                    node.getAttribute("id"),
                    target.getAttribute("id"),
                    target instanceof Text,
                    target instanceof Widget,
                );
                this.$.insertChildAfter(node.$, target.$.prevSibling);
            } else {
                console.log("Appending...", node.getAttribute("id"));
                this.$.append(node.$);
            }
        } else {
            console.log("Insert before nonwidget");
        }
    };

    appendChild = (node: INode) => {
        if (this.tail) {
            node.prevSibling = this.tail;
            this.tail.nextSibling = node;
        }
        this.tail = node;
        node.parentNode = this;
        node.mounted = true;
        if (isWidget(node)) {
            console.log("Appending...", node.getAttribute("id"));
            this.$.append(node.$);
        } else {
            console.log("Appending nonwidget");
        }
    };

    removeChild = (node: INode) => {
        const prevSibling = node.prevSibling;
        const nextSibling = node.nextSibling;
        if (prevSibling) {
            prevSibling.nextSibling = nextSibling;
        }
        if (nextSibling) {
            nextSibling.prevSibling = prevSibling;
        } else if (this.tail === node) {
            this.tail = prevSibling;
        }
        node.nextSibling = null;
        node.prevSibling = null;
        node.parentNode = null;
        node.mounted = false;
        if (isWidget(node)) {
            console.log("Removing child...", node.getAttribute("id"));
            this.$.remove(node.$);
        } else {
            console.log("Removing nonwidget");
        }
    };
}
