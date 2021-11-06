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
    const nextSibling = node.__next__;
    node.__prev__ = null;
    const prevSibling = node.__prev__;
    node.__prev__ = null;
    node.__parent__ = null;
    node.__mounted__ = false;
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
    node.__parent__ = self;
    if (anchor) {
        node.__prev__ = anchor.__prev__;
        node.__next__ = anchor;
        anchor.__prev__ = node;
    } else {
        if (state.tail) {
            node.__prev__ = state.tail;
            console.log(`>>>>>x ${JSON.stringify(state.tail, null, 2)}`);
            state.tail.__next__ = node;
        }
        state.tail = node;
    }
    node.__mounted__ = true;
}

export function appendChild(self: any, state: State, node: INode) {
    if (state.tail) {
        node.__prev__ = state.tail;
        state.tail.__next__ = node;
    }
    state.tail = node;
    node.__parent__ = self;
    node.__mounted__ = true;
}

export function removeChild(self: any, state: State, node: INode) {
    const prevSibling = node.__prev__;
    const nextSibling = node.__next__;
    if (prevSibling) {
        prevSibling.__next__ = nextSibling;
    }
    if (nextSibling) {
        nextSibling.__prev__ = prevSibling;
    } else if (state.tail === node) {
        state.tail = prevSibling;
    }
    node.__next__ = null;
    node.__prev__ = null;
    node.__parent__ = null;
    node.__mounted__ = false;
}

export const isClass = (cls: any): cls is IWidget => cls;
export const isWidget = (cls: any): cls is IWidget =>
    cls && !!cls.isGtkWidget && !!cls.isGObject;
export const isNode = (cls: any): cls is Node => cls && cls instanceof Node;
export const isText = (cls: any): cls is Text => cls && cls instanceof Text;
export const isChild = (cls: any): cls is IChild =>
    (isWidget(cls) || isClass(cls)) &&
    typeof (cls as any).__name__ === "string";
export const isComment = (cls: any): cls is Comment =>
    cls && cls instanceof Comment;
export const isStyle = (cls: any): cls is Style => cls && cls instanceof Style;

export const getNextWidget = (node: INode): IWidget | null => {
    let next = node.__next__;
    while (next) {
        if (isWidget(next) && next.__mounted__) {
            break;
        }
        next = next.__next__;
    }
    return isWidget(next) ? next : null;
};

export const getPrevWidget = (node: INode): INode | null => {
    let prev = node.__prev__;
    while (prev) {
        if (isWidget(prev) && prev.__mounted__) {
            break;
        }
        prev = prev.__prev__;
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
        if (x?.hasOwnProperty?.("id")) {
            res += ` id="${x.id}" `;
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

export const createLogger =
    (tag: any) =>
    (...msg: any) => {
        console.log(`${tag}`, ...msg);
    };
