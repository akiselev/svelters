import { Node, INode, Text, Child, Comment, Style } from "./nodes";
import type { IWidget } from "./widget";

export const isWidget = (cls: any): cls is IWidget =>
    !!cls?.isGtkWidget && !!cls?.isGObject && !!cls?.$;

export const getNextWidget = (node: INode): INode | null => {
    let next = node.nextSibling;
    while (next) {
        if (isWidget(next) && next.mounted) {
            break;
        }
        next = next.nextSibling;
    }
    return isWidget(next) ? next : null;
};

export const getPrevWidget = (node: INode): INode | null => {
    let prev = node.prevSibling;
    while (prev) {
        if (isWidget(prev) && prev.mounted) {
            break;
        }
        prev = prev.prevSibling;
    }
    return isWidget(prev) ? prev : null;
};

export const str = (x: any): string => {
    if (typeof x === "undefined") {
        return "{undefined}";
    } else if (x === null) {
        return "{null}";
    } else if (typeof x === "string") {
        return x;
    } else if (typeof x === "number") {
        return `${x}`;
    } else if (typeof x?.tagName === "string") {
        let res = `<${x.tagName}`;
        if (x?.$?.hasOwnProperty?.("id")) {
            res += ` id="${x.$.id}" `;
        }
        return `${res}>`;
    } else if (x instanceof Text) {
        return `{text: "${x.text}"}`;
    } else if (x instanceof Child) {
        return `{child: ${x.name}}`;
    } else if (x instanceof Comment) {
        return `{comment: ${x.comment}}`;
    } else if (x instanceof Style) {
        return `{comment: ${x.id}}`;
    } else {
        try {
            return `JSON(${JSON.stringify(x)})`;
        } catch {
            if (Array.isArray(x)) {
                return `{unknown array}`;
            } else if (typeof x === "object") {
                return "{unknown object}";
            }
            return "{unknown}";
        }
    }
};

export const createToString = (x: any) => () => str(x);

export const createLogger =
    (_: any) =>
    (...msg: any) => {
        console.log(`${str(_)}`, ...msg);
    };
