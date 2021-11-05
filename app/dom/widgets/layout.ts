import { Node, INode, Text, IWidget } from "../nodes";
import { createLogger, createToString, str, unmountNode } from "../utils";
import { Widget } from "./widget";
import {
    isWidget,
    isChild,
    getNextWidget,
    insertChildBefore,
    appendChild,
    removeChild,
} from "../utils";

export interface ILayout<T> {
    onInsertWidgetAfter(self: T, child: any, anchor?: any): void;
    onAppendWidget(self: T, child: any): void;
    onRemoveWidget(self: T, child: any): void;
    children?: {
        [name: string]: (self: T, child: any, remove?: boolean) => void;
    };
}

interface ILayoutWidget extends IWidget {
    get tail(): INode | null;
}

export function LayoutWidget<T = any>(
    tagName: string,
    $: T,
    hooks: ILayout<T>,
): ILayoutWidget {
    const _ = Widget(tagName, $, hooks.children);
    const state = { head: null as INode | null, tail: null as INode | null };

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

        onUnmount() {
            log(`=> onUnmount`);
            unmountNode(state.head);
            state.head = null;
            unmountNode(state.tail);
            state.tail = null;
        },

        insertBefore(node: INode, anchor?: INode) {
            log(`=> insert`, str(node), `before`, str(anchor));
            insertChildBefore(self, state, node, anchor);

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
            appendChild(self, state, node);

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
            removeChild(self, state, node);

            if (isWidget(node)) {
                hooks.onRemoveWidget(_.$, node.$);
            } else if (isChild(node)) {
                const handler = hooks.children?.[node.__name__];
                if (handler) {
                    handler(_.$, node.$, true);
                } else {
                    throw new Error(
                        `Widget ${tagName} does not support "${node.__name__}" children`,
                    );
                }
            }
            node?.onUnmount?.();
        },
    };
    return self;
}
