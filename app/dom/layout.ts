import { Node, INode, Text } from "./nodes";
import { createLogger, createToString, str } from "./utils";
import { WidgetConstructor, Widget, IWidget } from "./widget";
import { isWidget, getPrevWidget, getNextWidget } from "./utils";

interface IBox {
    insertChildAfter(child: any, anchor: any): void;
    append(child: any): void;
    prepend(child: any): void;
    remove(child: any): void;
}

export function BoxWidget<T>(tagName: string, $: T) {
    const _ = Widget(tagName, $);
    const state = { tail: null as IWidget | null };

    const log = createLogger(_);

    const self = {
        ..._,
        insertBefore(node: INode, anchor?: INode) {
            log(`=> insert`, str(node), `before`, str(anchor));
            node.parentNode = self;
            if (anchor) {
                node.prevSibling = anchor.prevSibling;
                node.nextSibling = anchor;
                anchor.prevSibling = node;
            } else {
                if (state.tail) {
                    node.prevSibling = state.tail;
                    state.tail.nextSibling = node;
                }
                state.tail = node;
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
                    _.$.insertChildAfter(node.$, target.$.prevSibling);
                } else {
                    console.log("Appending...", node.getAttribute("id"));
                    _.$.append(node.$);
                }
            } else {
                console.log("Insert before nonwidget");
            }
        },

        appendChild(node: INode) {
            log(`=> append`, str(node));
            if (state.tail) {
                node.prevSibling = state.tail;
                state.tail.nextSibling = node;
            }
            state.tail = node;
            node.parentNode = self;
            node.mounted = true;
            if (isWidget(node)) {
                console.log("Appending...", node.getAttribute("id"));
                _.$.append(node.$);
            } else {
                console.log("Appending nonwidget");
            }
        },

        removeChild(node: INode) {
            log(`=> remove`, str(node));
            const prevSibling = node.prevSibling;
            const nextSibling = node.nextSibling;
            if (prevSibling) {
                prevSibling.nextSibling = nextSibling;
            }
            if (nextSibling) {
                nextSibling.prevSibling = prevSibling;
            } else if (state.tail === node) {
                state.tail = prevSibling;
            }
            node.nextSibling = null;
            node.prevSibling = null;
            node.parentNode = null;
            node.mounted = false;
            if (isWidget(node)) {
                console.log("Removing child...", node.getAttribute("id"));
                _.$.remove(node.$);
            } else {
                console.log("Removing nonwidget");
            }
        },

        get tail() {
            return state.tail;
        },

        set tail(value: IWidget | null) {
            log(`=> tail:`, str(value));
            state.tail = value;
        },
    };
    return self;
}
