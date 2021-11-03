import { INode, IWidget } from "../nodes";
import { createLogger, createToString, str } from "../utils";

export function Widget<T>(tagName: string, $: any): IWidget {
    const _ = {
        $,
        tagName,
        mounted: false,
        parentNode: null as INode | null,
        nextSibling: null as INode | null,
        prevSibling: null as INode | null,
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

        set parentNode(value: INode | null) {
            log(`=> parentNode:`, str(value));
            _.parentNode = value;
        },

        get nextSibling() {
            return _.nextSibling;
        },

        set nextSibling(value: INode | null) {
            log(`=> nextSibling:`, str(value));
            _.nextSibling = value;
        },

        get prevSibling() {
            return _.prevSibling;
        },

        set prevSibling(value: INode | null) {
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

        getRootNode() {
            return document;
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
    } as IWidget;
}
