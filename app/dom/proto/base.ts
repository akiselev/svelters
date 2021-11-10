import {
    createLogger,
    createToString,
    str,
    insertChildBefore,
    appendChild,
    removeChild,
    isChild,
    unmountNode,
    isWidget,
    getNextWidget,
} from "./utils";

/// <reference path="../../../node_modules/typescript/lib/lib.es5.d.ts" />

type Hooks = {
    [name: string]: (self: any, child: any, remove?: boolean) => void;
};

export const initBaseNode = (name: string, CLS: any) => {
    const log = createLogger(name);
    Object.defineProperty(CLS.prototype, "svTagName", {
        value: name,
    });
    Object.defineProperty(CLS.prototype, "setAttribute", {
        value: function (attr: string, value: any) {
            if (attr === "class") {
                console.log("setting classes: ", value);
                this.svSetAttribute("cssClasses", value.split(" "));
            } else {
                this.svSetAttribute(attr, value);
            }
        },
    });
    Object.defineProperty(CLS.prototype, "getAttribute", {
        value: function (attr: string) {
            if (attr === "class") {
                return this.svGetAttribute("cssClasses").join(" ");
            } else {
                return this.svGetAttribute(attr);
            }
        },
    });
    Object.defineProperty(CLS.prototype, "getRootNode", {
        value: function () {
            return document;
        },
    });
    Object.defineProperty(CLS.prototype, "toString", {
        value: createToString(name),
    });
    Object.defineProperty(CLS.prototype, "classList", {
        get() {
            return {
                add(name: string): void {
                    log(`=> addClass [${name}]`);
                    this.addClass(name);
                },
                remove(name: string): void {
                    log(`=> removeClass [${name}]`);
                    this.removeClass(name);
                },
            };
        },
    });

    Object.defineProperty(CLS.prototype, "svLog", {
        value: log,
    });
    Object.defineProperty(CLS.prototype, "svName", {
        get() {
            return this.svExtra.name;
        },

        set(value: boolean) {
            log(`=> name:`, str(value));
            this.svExtra.name = value;
        },
    });
    Object.defineProperty(CLS.prototype, "svMounted", {
        get() {
            return this.svExtra.mounted;
        },

        set(value: boolean) {
            log(`${value ? "" : "un"}mounted`);
            this.svExtra.mounted = value;
        },
    });
    Object.defineProperty(CLS.prototype, "parentNode", {
        get() {
            return this.svExtra.parentNode;
        },

        set(value: any) {
            log(`=> parentNode:`, str(value));
            this.svExtra.parentNode = value;
        },
    });
    Object.defineProperty(CLS.prototype, "svNextSibling", {
        get() {
            return this.svExtra.nextSibling;
        },

        set(value: any) {
            log(`=> nextSibling:`, str(value));
            this.svExtra.nextSibling = value;
        },
    });
    Object.defineProperty(CLS.prototype, "svPrevSibling", {
        get() {
            return this.svExtra.prevSibling;
        },

        set(value: any) {
            log(`=> prevSibling:`, str(value));
            this.svExtra.prevSibling = value;
        },
    });
};

export const initSingleton = (CLS: any, hooks: Hooks = {}) => {
    Object.defineProperty(CLS.prototype, "insertBefore", {
        value: function (node: any, anchor: any) {
            if (isChild(node)) {
                const handler = hooks[node.svName];
                if (handler) {
                    handler(this, node);
                } else {
                    throw new Error(
                        `Widget ${this.svTagName} does not support "${node.svName}" children`,
                    );
                }
            } else {
                this.child?.onUnmount?.();
                this.child = node;
                this.svChild = node;
            }
        },
    });
    Object.defineProperty(CLS.prototype, "appendChild", {
        value: function (node: any) {
            if (isChild(node)) {
                const handler = hooks[node.svName];
                if (handler) {
                    handler(this, node);
                } else {
                    throw new Error(
                        `Widget ${this.svTagName} does not support "${node.svName}" children`,
                    );
                }
            } else {
                this.child?.onUnmount?.();
                this.child = node;
                this.svChild = node;
            }
        },
    });
    Object.defineProperty(CLS.prototype, "removeChild", {
        value: function (node: any) {
            if (isChild(node)) {
                const handler = hooks[node.svName];
                if (handler) {
                    handler(this, node, true);
                } else {
                    throw new Error(
                        `Widget ${this.svTagName} does not support "${node.svName}" children`,
                    );
                }
            } else if (this.svChild === node) {
                this.child?.onUnmount?.();
                this.child = null;
                this.svChild = null;
            }
        },
    });
    Object.defineProperty(CLS.prototype, "onUnmount", {
        value: function (node: any) {
            this.child?.onUnmount?.();
            this.child = null;
            this.svChild = null;
        },
    });
    Object.defineProperty(CLS.prototype, "svChild", {
        get() {
            return this.svExtra.child;
        },

        set(value: any) {
            this.svLog(`=> prevSibling:`, str(value));
            this.svExtra.child = value;
        },
    });
};

export const initWidget = (CLS: any, hooks: Hooks) => {
    Object.defineProperty(CLS.prototype, "insertBefore", {
        value: function (node: any, anchor: any) {
            insertChildBefore(this, node, anchor);

            if (isChild(node)) {
                const handler = hooks[node.svName];
                if (handler) {
                    handler(this, node);
                } else {
                    throw new Error(
                        `Widget ${this.svTagName} does not support "${node.svName}" children`,
                    );
                }
            }
        },
    });
    Object.defineProperty(CLS.prototype, "appendChild", {
        value: function (node: any) {
            appendChild(this, node);

            if (isChild(node)) {
                const handler = hooks[node.svName];
                if (handler) {
                    handler(this, node);
                } else {
                    throw new Error(
                        `Widget ${this.svTagName} does not support "${node.svName}" children`,
                    );
                }
            }
        },
    });
    Object.defineProperty(CLS.prototype, "removeChild", {
        value: function (node: any) {
            removeChild(this, node);

            if (isChild(node)) {
                const handler = hooks[node.svName];
                if (handler) {
                    handler(this, node, true);
                } else {
                    throw new Error(
                        `Widget ${this.svTagName} does not support "${node.svName}" children`,
                    );
                }
            }
            node?.onUnmount?.();
        },
    });
    Object.defineProperty(CLS.prototype, "onUnmount", {
        value: function (node: any) {
            unmountNode(this.svTail);
        },
    });
    Object.defineProperty(CLS.prototype, "svChild", {
        get() {
            return this.svExtra.child;
        },

        set(value: any) {
            this.svLog(`=> prevSibling:`, str(value));
            this.svExtra.child = value;
        },
    });
    Object.defineProperty(CLS.prototype, "svTail", {
        get() {
            return this.svExtra.tail;
        },

        set(value: any) {
            this.svLog(`=> prevSibling:`, str(value));
            this.svExtra.tail = value;
        },
    });
};

export interface ILayout {
    onInsertWidgetAfter(self: any, child: any, anchor?: any): void;
    onAppendWidget(self: any, child: any): void;
    onRemoveWidget(self: any, child: any): void;
    children?: {
        [name: string]: (self: any, child: any, remove?: boolean) => void;
    };
}

export const initLayout = (CLS: any, hooks: ILayout) => {
    Object.defineProperty(CLS.prototype, "insertBefore", {
        value: function (node: any, anchor: any) {
            insertChildBefore(this, node, anchor);

            if (isWidget(node)) {
                let target = getNextWidget(node);
                if (isWidget(target)) {
                    hooks.onInsertWidgetAfter(this, node, target.prevSibling);
                } else {
                    hooks.onAppendWidget(this, node);
                }
            } else if (isChild(node)) {
                const handler = hooks.children?.[node.svName];
                if (handler) {
                    handler(this, node);
                } else {
                    throw new Error(
                        `Widget ${this.svTagName} does not support "${node.svName}" children`,
                    );
                }
            }
        },
    });
    Object.defineProperty(CLS.prototype, "appendChild", {
        value: function (node: any) {
            appendChild(this, node);
            if (isWidget(node)) {
                hooks.onAppendWidget(this, node);
            } else if (isChild(node)) {
                const handler = hooks.children?.[node.svName];
                if (handler) {
                    handler(this, node);
                } else {
                    throw new Error(
                        `Widget ${this.svTagName} does not support "${node.svName}" children`,
                    );
                }
            }
        },
    });
    Object.defineProperty(CLS.prototype, "removeChild", {
        value: function (node: any) {
            removeChild(this, node);
            if (isWidget(node)) {
                hooks.onRemoveWidget(this, node);
            } else if (isChild(node)) {
                const handler = hooks.children?.[node.svName];
                if (handler) {
                    handler(this, node, true);
                } else {
                    throw new Error(
                        `Widget ${this.svTagName} does not support "${node.svName}" children`,
                    );
                }
            }
            node?.onUnmount?.();
        },
    });
    Object.defineProperty(CLS.prototype, "onUnmount", {
        value: function (node: any) {
            unmountNode(this.svTail);
        },
    });
    Object.defineProperty(CLS.prototype, "svChild", {
        get() {
            return this.svExtra.child;
        },

        set(value: any) {
            this.svLog(`=> prevSibling:`, str(value));
            this.svExtra.child = value;
        },
    });
    Object.defineProperty(CLS.prototype, "svTail", {
        get() {
            return this.svExtra.tail;
        },

        set(value: any) {
            this.svLog(`=> prevSibling:`, str(value));
            this.svExtra.tail = value;
        },
    });
};
