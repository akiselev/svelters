import { Node, INode, Text, IContainer, IWidget } from "../nodes";
import { createLogger, createToString, str } from "../utils";
import { Widget } from "./widget";
import { isWidget, isChild, getNextWidget } from "../utils";

export interface ILayout<T> {
    onInsertWidgetAfter(self: T, child: any, anchor?: any): void;
    onAppendWidget(self: T, child: any): void;
    onRemoveWidget(self: T, child: any): void;
    children?: {
        [name: string]: (self: T, child: any | null) => void;
    };
}

interface ILayoutWidget extends IWidget, IContainer {
    get tail(): INode | null;
}

export function LayoutWidget<T = any>(
    tagName: string,
    $: T,
    hooks: ILayout<T>,
): ILayoutWidget {
    const _ = Widget(tagName, $);
    const state = { tail: null as INode | null };

    const log = createLogger(_);

    const self = {
        ..._,

        get tail() {
            return state.tail;
        },

        set tail(value: INode | null) {
            log(`=> tail:`, str(value));
            state.tail = value;
        },

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

            if (isWidget(node)) {
                let target = getNextWidget(node);
                if (isWidget(target)) {
                    hooks.onInsertWidgetAfter(
                        _.$,
                        node.$,
                        target.$.prevSibling,
                    );
                } else {
                    hooks.onAppendWidget(_.$, node.$);
                }
            } else if (isChild(node)) {
                const handler = hooks.children?.[node.__name__];
                if (handler) {
                    handler(_.$, node);
                } else {
                    throw new Error(
                        `Widget ${tagName} does not support "${node.__name__}" children`,
                    );
                }
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
                hooks.onAppendWidget(_.$, node.$);
            } else if (isChild(node)) {
                const handler = hooks.children?.[node.__name__];
                if (handler) {
                    handler(_.$, node);
                } else {
                    throw new Error(
                        `Widget ${tagName} does not support "${node.__name__}" children`,
                    );
                }
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
                hooks.onRemoveWidget(_.$, node.$);
            } else if (isChild(node)) {
                const handler = hooks.children?.[node.__name__];
                if (handler) {
                    handler(_.$, null);
                } else {
                    throw new Error(
                        `Widget ${tagName} does not support "${node.__name__}" children`,
                    );
                }
            }
        },
    };
    return self;
}
