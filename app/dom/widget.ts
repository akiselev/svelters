import { INode } from "./nodes";
import { createLogger, createToString, str } from "./utils";
export interface IGtkWidget {
    isGObject?: boolean;
    isGtkWidget?: boolean;
    addClass(className: string): void;
    removeClass(className: string): void;
    setAttribute(name: string, value: any): void;
    getAttribute(name: string): any;
    removeAttribute(name: string): void;
    addEventListener(type: string, listener: (event: any) => void): void;
    removeEventListener(type: string, listener: (event: any) => void): void;
}

export interface WidgetConstructor<T = {}> {
    new (...args: Array<any>): T & IGtkWidget;
}

export interface IWidget {
    tagName: string;
    mounted: boolean;
    get isGtkWidget(): boolean;
    get isGObject(): boolean;

    nextSibling: any;
    prevSibling: any;
    parentNode: any;

    get $(): any;
}

export function Widget<T>(tagName: string, $: any) {
    const _ = {
        $,
        tagName,
        mounted: false,
        parentNode: null as IWidget | null,
        nextSibling: null as IWidget | null,
        prevSibling: null as IWidget | null,
    } as any;

    const log = createLogger(_);

    return {
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
            log(`${value ? "" : "un"}mounted`);
            _.mounted = value;
        },

        get parentNode() {
            return _.parentNode;
        },

        set parentNode(value: IWidget | null) {
            log(`=> parentNode:`, str(value));
            _.parentNode = value;
        },

        get nextSibling() {
            return _.nextSibling;
        },

        set nextSibling(value: IWidget | null) {
            log(`=> nextSibling:`, str(value));
            _.nextSibling = value;
        },

        get prevSibling() {
            return _.prevSibling;
        },

        set prevSibling(value: IWidget | null) {
            log(`=> prevSibling:`, str(value));
            _.prevSibling = value;
        },

        toString: createToString(_),

        setAttribute(name: string, value: any) {
            log(`=> ${name} =`, str(value));
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
            log(`=> delete [${name}]`);
            _.$.removeAttribute(name);
        },

        addEventListener(name: string, callback: (event: any) => void) {
            log(`=> connect [${name}]`);
            _.$.addEventListener(name, callback);
        },

        removeEventListener(name: string, callback: (event: any) => void) {
            log(`=> disconnect [${name}]`);
            _.$.removeEventListener(name, callback);
        },

        classList: {
            add(name: string): void {
                log(`=> addClass [${name}]`);
                _.$.addClass(name);
            },
            remove(name: string): void {
                log(`=> removeClass [${name}]`);
                _.$.removeClass(name);
            },
        },
    };
}
