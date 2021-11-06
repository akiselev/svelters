import {
    Node,
    INode,
    Text,
    Child,
    Comment,
    Style,
    IWidget,
    IChild,
} from "./nodes";

interface State {
    tail: INode | null;
}

export function unmountNode(node: any) {
    if (!node) return;
    const nextSibling = node.nextSibling;
    node.nextSibling = null;
    const prevSibling = node.prevSibling;
    node.prevSibling = null;
    node.parentNode = null;
    node.mounted = false;
    if (nextSibling) {
        unmountNode(nextSibling);
    }
    if (prevSibling) {
        unmountNode(prevSibling);
    }
}

export function insertChildBefore(
    self: any,
    state: State,
    node: INode,
    anchor?: INode,
) {
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
}

export function appendChild(self: any, state: State, node: INode) {
    if (state.tail) {
        node.prevSibling = state.tail;
        state.tail.nextSibling = node;
    }
    state.tail = node;
    node.parentNode = self;
    node.mounted = true;
}

export function removeChild(self: any, state: State, node: INode) {
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
}

export const isClass = (cls: any): cls is IWidget => cls && !!cls.$;
export const isWidget = (cls: any): cls is IWidget =>
    cls && !!cls.isGtkWidget && !!cls.isGObject && !!cls.$;
export const isNode = (cls: any): cls is Node => cls && cls instanceof Node;
export const isText = (cls: any): cls is Text => cls && cls instanceof Text;
export const isChild = (cls: any): cls is IChild =>
    (isWidget(cls) || isClass(cls)) &&
    typeof (cls as any).__name__ === "string";
export const isComment = (cls: any): cls is Comment =>
    cls && cls instanceof Comment;
export const isStyle = (cls: any): cls is Style => cls && cls instanceof Style;

export const getNextWidget = (node: INode): IWidget | null => {
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
    } else if (isText(x)) {
        return `{text: "${x.text}"}`;
    } else if (isChild(x)) {
        return `{child: ${x.__name__}}`;
    } else if (isComment(x)) {
        return `{comment: ${x.comment}}`;
    } else if (isStyle(x)) {
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
