import { INode, IWidget } from "../nodes";
import {
    createLogger,
    createToString,
    str,
    insertChildBefore,
    appendChild,
    removeChild,
    isChild,
    unmountNode,
} from "../utils";

type Hooks<T> = {
    [name: string]: (self: T, child: any, remove?: boolean) => void;
};

export function Widget<T>(
    tagName: string,
    $: T,
    hooks: Hooks<T> = {},
): IWidget {
    const _ = {
        $,
        tagName,
        mounted: false,
        parentNode: null as INode | null,
        nextSibling: null as INode | null,
        prevSibling: null as INode | null,
    } as any;
    const state = { tail: null as INode | null };

    const log = createLogger(_);

    const self = {
        get $() {
            return _.$;
        },

        get tagName() {
            return _.tagName;
        },

        get isGtkWidget() {
            return _.$.isGtkWidget;
        },

        get isGObject() {
            return _.$.isGObject;
        },

        get mounted() {
            return _.mounted;
        },

        set mounted(value: boolean) {
            //$log(`${value ? "" : "un"}mounted`);
            _.mounted = value;
        },

        get parentNode() {
            return _.parentNode;
        },

        set parentNode(value: INode | null) {
            //$log(`=> parentNode:`, str(value));
            _.parentNode = value;
        },

        get nextSibling() {
            return _.nextSibling;
        },

        set nextSibling(value: INode | null) {
            //$log(`=> nextSibling:`, str(value));
            _.nextSibling = value;
        },

        get prevSibling() {
            return _.prevSibling;
        },

        set prevSibling(value: INode | null) {
            //$log(`=> prevSibling:`, str(value));
            _.prevSibling = value;
        },

        onUnmount() {
            //$log(`=> onUnmount`);
            unmountNode(state.tail);
            state.tail = null;
        },

        toString: createToString(_),

        setAttribute(name: string, value: any) {
            //$log(`=> ${name} =`, str(value));
            if (name === "class") {
                console.log("setting classes: ", value);
                _.$.setAttribute("cssClasses", value.split(" "));
            } else {
                _.$.setAttribute(name, value);
            }
        },

        getAttribute(name: string): any {
            if (name === "class") {
                return _.$.getAttribute("cssClasses").join(" ");
            } else {
                return _.$.getAttribute(name);
            }
        },

        removeAttribute(name: string) {
            //$log(`=> delete [${name}]`);
            _.$.removeAttribute(name);
        },

        addEventListener(name: string, callback: (event: any) => void) {
            //$log(`=> connect [${name}]`);
            _.$.addEventListener(name, callback);
        },

        removeEventListener(name: string, callback: (event: any) => void) {
            //$log(`=> disconnect [${name}]`);
            _.$.removeEventListener(name, callback);
        },

        insertBefore(node: INode, anchor?: INode) {
            //$log(`=> insert`, str(node), `before`, str(anchor));
            insertChildBefore(self, state, node, anchor);

            if (isChild(node)) {
                if (node.__name__ == "controller") {
                    console.log("adding controller....");
                    _.$.addController(node.$);
                } else {
                    const handler = hooks[node.__name__];
                    if (handler) {
                        handler(_.$, node);
                    } else {
                        throw new Error(
                            `Widget ${tagName} does not support "${node.__name__}" children`,
                        );
                    }
                }
            }
        },

        appendChild(node: INode) {
            //$log(`=> append`, str(node));
            appendChild(self, state, node);

            if (isChild(node)) {
                if (node.__name__ == "controller") {
                    console.log("adding controller....");
                    _.$.addController(node.$);
                } else {
                    const handler = hooks[node.__name__];
                    if (handler) {
                        handler(_.$, node);
                    } else {
                        throw new Error(
                            `Widget ${tagName} does not support "${node.__name__}" children`,
                        );
                    }
                }
            }
        },

        removeChild(node: INode) {
            //$log(`=> remove`, str(node));
            removeChild(self, state, node);

            if (isChild(node)) {
                if (node.__name__ == "controller") {
                    console.log("removing controller....");
                    _.$.removeController(node.$);
                } else {
                    const handler = hooks[node.__name__];
                    if (handler) {
                        handler(_.$, node, true);
                    } else {
                        throw new Error(
                            `Widget ${tagName} does not support "${node.__name__}" children`,
                        );
                    }
                }
            }
            node?.onUnmount?.();
        },

        getRootNode() {
            return document;
        },

        classList: {
            add(name: string): void {
                //$log(`=> addClass [${name}]`);
                _.$.addClass(name);
            },
            remove(name: string): void {
                //$log(`=> removeClass [${name}]`);
                _.$.removeClass(name);
            },
        },
    } as IWidget;
    return self;
}
