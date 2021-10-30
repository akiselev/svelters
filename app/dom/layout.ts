import { Node, INode, Text } from "./nodes";
import { WidgetConstructor, Widget, isWidget, getPrevWidget } from "./widget";

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
        if (anchor) {
            node.prevSibling = anchor.prevSibling;
            node.nextSibling = anchor;
            anchor.prevSibling = node;
            node.parentNode = this;
        } else {
            if (this.tail) {
                node.prevSibling = this.tail;
                this.tail.nextSibling = node;
            }
            node.parentNode = this;
            this.tail = node;
        }
        node.mounted = true;
        if (isWidget(node)) {
            console.log("Insert before...");
            let target = getPrevWidget(node);
            if (isWidget(target)) {
                console.log(
                    "Insert after target: ",
                    node.getAttribute("id"),
                    target.getAttribute("id"),
                    target instanceof Text,
                    target instanceof Widget,
                );
                this.$.insertChildAfter(node.$, target.$);
            } else {
                console.log("Prepending...", node.getAttribute("id"));
                this.$.prepend(node.$);
            }
        }
    };

    appendChild = (node: INode) => {
        if (this.tail) {
            node.prevSibling = this.tail;
            this.tail.nextSibling = node;
        }
        node.parentNode = this;
        node.mounted = true;
        if (isWidget(node)) {
            console.log("Appending...", node.getAttribute("id"));
            this.$.append(node.$);
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
        }
    };
}
