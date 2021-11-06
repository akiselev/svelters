import { Constructor, INode, IWidget } from "../nodes";
import {
    createLogger,
    str,
    insertChildBefore,
    appendChild,
    removeChild,
    isChild,
    unmountNode,
} from "../utils";

type Hooks = {
    [name: string]: (self: any, child: any, remove?: boolean) => void;
};

export function WidgetCls<T extends Constructor<GtkWidget>>(
    tagName: string,
    $: T,
    hooks: Hooks = {},
): Constructor<IWidget> {
    const state = { tail: null as INode | null };

    const log = createLogger(`<${tagName}>`);
    return class extends $ {
        private __state__ = {
            mounted: false,
            parentNode: null as INode | null,
            nextSibling: null as INode | null,
            prevSibling: null as INode | null,
        };

        constructor(...args: any) {
            super(...args);
            this.classList = {
                add(name: string): void {
                    log(`=> addClass [${name}]`);
                    super.addClass(name);
                },
                remove(name: string): void {
                    log(`=> removeClass [${name}]`);
                    super.removeClass(name);
                },
            };
        }

        get isGtkWidget() {
            return true;
        }

        get tagName() {
            return tagName;
        }

        get __mounted__() {
            return this.__state__.mounted;
        }

        set __mounted__(value: boolean) {
            log(`${value ? "" : "un"}mounted`);
            this.__state__.mounted = value;
        }

        get __parent__() {
            return this.__state__.parentNode;
        }

        set __parent__(value: INode | null) {
            log(`=> parentNode:`, str(value));
            this.__state__.parentNode = value;
        }

        get __next__() {
            return this.__state__.nextSibling;
        }

        set __next__(value: INode | null) {
            log(`=> __next__:`, str(value));
            this.__state__.nextSibling = value;
        }

        get __prev__() {
            return this.__state__.prevSibling;
        }

        set __prev__(value: INode | null) {
            log(`=> prevSibling:`, str(value));
            this.__state__.prevSibling = value;
        }

        onUnmount() {
            log(`=> onUnmount`);
            unmountNode(state.tail);
            state.tail = null;
        }

        toString = () => str(this);

        setAttribute = (name: string, value: any) => {
            log(`=> ${name} =`, str(value));
            if (name === "class") {
                console.log("setting classes: ", value);
                super.setAttribute("cssClasses", value.split(" "));
            } else {
                super.setAttribute(name, value);
            }
        };

        getAttribute = (name: string): any => {
            if (name === "class") {
                return super.getAttribute("cssClasses").join(" ");
            } else {
                return super.getAttribute(name);
            }
        };

        removeAttribute = (name: string) => {
            log(`=> delete [${name}]`);
            super.removeAttribute(name);
        };

        addEventListener = (name: string, callback: (event: any) => void) => {
            log(`=> connect [${name}]`);
            super.addEventListener(name, callback);
        };

        removeEventListener = (
            name: string,
            callback: (event: any) => void,
        ) => {
            log(`=> disconnect [${name}]`);
            super.removeEventListener(name, callback);
        };

        insertBefore = (node: INode, anchor?: INode) => {
            log(`=> insert`, str(node), `before`, str(anchor));
            insertChildBefore(this, state, node, anchor);

            if (isChild(node)) {
                if (node.__name__ == "controller") {
                    console.log("adding controller....");
                    super.addController(node);
                } else {
                    const handler = hooks[node.__name__];
                    if (handler) {
                        handler(this, node);
                    } else {
                        throw new Error(
                            `Widget ${tagName} does not support "${node.__name__}" children`,
                        );
                    }
                }
            }
        };

        appendChild = (node: INode) => {
            log(`=> append`, str(node));
            appendChild(this, state, node);

            if (isChild(node)) {
                if (node.__name__ == "controller") {
                    console.log("adding controller....");
                    super.addController(node);
                } else {
                    const handler = hooks[node.__name__];
                    if (handler) {
                        handler(this, node);
                    } else {
                        throw new Error(
                            `Widget ${tagName} does not support "${node.__name__}" children`,
                        );
                    }
                }
            }
        };

        removeChild = (node: INode) => {
            log(`=> remove`, str(node));
            removeChild(this, state, node);

            if (isChild(node)) {
                if (node.__name__ == "controller") {
                    console.log("removing controller....");
                    super.removeController(node);
                } else {
                    const handler = hooks[node.__name__];
                    if (handler) {
                        handler(this, node, true);
                    } else {
                        throw new Error(
                            `Widget ${tagName} does not support "${node.__name__}" children`,
                        );
                    }
                }
            }
            node?.onUnmount?.();
        };

        getRootNode() {
            return document;
        }

        classList: any;
    };
}
