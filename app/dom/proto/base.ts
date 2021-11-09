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

/// <reference path="../../../node_modules/typescript/lib/lib.es5.d.ts" />

export const initBaseNode = (name: string, CLS: any) => {
    const log = createLogger(name);
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
    Object.defineProperty(CLS.prototype, "log", {
        value: log,
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

    Object.defineProperty(CLS.prototype, "$", {
        get() {
            return this;
        },
    });
    Object.defineProperty(CLS.prototype, "name", {
        get() {
            return this.svExtra.name;
        },

        set(value: boolean) {
            log(`=> name:`, str(value));
            this.svExtra.name = value;
        },
    });
    Object.defineProperty(CLS.prototype, "mounted", {
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
    Object.defineProperty(CLS.prototype, "nextSibling", {
        get() {
            return this.svExtra.nextSibling;
        },

        set(value: any) {
            log(`=> nextSibling:`, str(value));
            this.svExtra.nextSibling = value;
        },
    });
    Object.defineProperty(CLS.prototype, "prevSibling", {
        get() {
            return this.svExtra.prevSibling;
        },

        set(value: any) {
            log(`=> prevSibling:`, str(value));
            this.svExtra.prevSibling = value;
        },
    });
};

export const initSingleton = (CLS: any) => {
    Object.defineProperty(CLS.prototype, "insertBefore", {
        value: function (node: any, anchor: any) {
            this.child = node;
            this.svChild = node;
        },
    });
    Object.defineProperty(CLS.prototype, "appendChild", {
        value: function (node: any) {
            this.child = node;
            this.svChild = node;
        },
    });
    Object.defineProperty(CLS.prototype, "removeChild", {
        value: function (node: any) {
            if (this.svChild === node) {
                this.child = null;
                this.svChild = null;
            }
        },
    });
    Object.defineProperty(CLS.prototype, "child", {
        get() {
            return this.svExtra.child;
        },

        set(value: any) {
            this.svLog(`=> prevSibling:`, str(value));
            this.svExtra.child = value;
        },
    });
};
