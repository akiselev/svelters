import { Node, INode, Text, IWidget, Constructor } from "../nodes";
import { createLogger, str, unmountNode } from "../utils";
import { WidgetCls } from "./widget";
import {
    isWidget,
    isChild,
    getNextWidget,
    insertChildBefore,
    appendChild,
    removeChild,
} from "../utils";

export interface ILayout {
    onInsertWidgetAfter(self: any, child: any, anchor?: any): void;
    onAppendWidget(self: any, child: any): void;
    onRemoveWidget(self: any, child: any): void;
    children?: {
        [name: string]: (self: any, child: any, remove?: boolean) => void;
    };
}

interface ILayoutWidget extends IWidget {
    get tail(): INode | null;
}

export function LayoutWidget<T extends Constructor<GtkWidget>>(
    tagName: string,
    $: T,
    hooks: ILayout,
): Constructor<ILayoutWidget> {
    const _ = WidgetCls(tagName, $, hooks.children);

    const log = createLogger(`<${tagName}>`);
    return class extends _ {
        __state__ = { tail: null as INode | null };

        constructor(...args: any) {
            super(...args);
        }

        get tail() {
            return this.__state__.tail;
        }

        set tail(value: INode | null) {
            log(`=> tail:`, str(value));
            this.__state__.tail = value;
        }

        onUnmount() {
            log(`=> onUnmount`);
            unmountNode(this.__state__.tail);
            this.__state__.tail = null;
        }

        insertBefore = (node: INode, anchor?: INode) => {
            log(`=> insert`, str(node), `before`, str(anchor));
            insertChildBefore(this, this.__state__, node, anchor);

            if (isWidget(node)) {
                let target = getNextWidget(node);
                if (isWidget(target)) {
                    hooks.onInsertWidgetAfter(this, node, target.__prev__);
                } else {
                    hooks.onAppendWidget(this, node);
                }
            } else if (isChild(node)) {
                const handler = hooks.children?.[node.__name__];
                if (handler) {
                    handler(this, node);
                } else {
                    throw new Error(
                        `Widget ${tagName} does not support "${node.__name__}" children`,
                    );
                }
            }
        };

        appendChild = (node: INode) => {
            log(`=> append`, str(node));
            appendChild(this, this.__state__, node);

            if (isWidget(node)) {
                hooks.onAppendWidget(this, node);
            } else if (isChild(node)) {
                const handler = hooks.children?.[node.__name__];
                if (handler) {
                    handler(this, node);
                } else {
                    throw new Error(
                        `Widget ${tagName} does not support "${node.__name__}" children`,
                    );
                }
            }
        };

        removeChild = (node: INode) => {
            log(`=> remove`, str(node));
            removeChild(this, this.__state__, node);

            if (isWidget(node)) {
                hooks.onRemoveWidget(this, node);
            } else if (isChild(node)) {
                const handler = hooks.children?.[node.__name__];
                if (handler) {
                    handler(this, node, true);
                } else {
                    throw new Error(
                        `Widget ${tagName} does not support "${node.__name__}" children`,
                    );
                }
            }
            node?.onUnmount?.();
        };
    };
}
