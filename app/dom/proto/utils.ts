import { Node, INode, Text, Comment, Style, IWidget, IChild } from "../nodes";

export function unmountNode(node: any) {
    if (!node) return;
    const svNextSibling = node.svNextSibling;
    node.svNextSibling = null;
    const svPrevSibling = node.svPrevSibling;
    node.svPrevSibling = null;
    node.parentNode = null;
    node.svMounted = false;
    if (svNextSibling) {
        unmountNode(svNextSibling);
    }
    if (svPrevSibling) {
        unmountNode(svPrevSibling);
    }
}

export function insertChildBefore(self: any, node: INode, anchor?: INode) {
    self.svLog(`=> insertChildBefore(${node}, ${anchor})`);
    node.parentNode = self;
    if (anchor) {
        node.svPrevSibling = anchor.svPrevSibling;
        node.svNextSibling = anchor;
        anchor.svPrevSibling = node;
    } else {
        if (self.svExtra.tail) {
            node.svPrevSibling = self.svExtra.tail;
            self.svExtra.tail.svNextSibling = node;
        }
        self.svExtra.tail = node;
    }
    node.svMounted = true;
}

export function appendChild(self: any, node: INode) {
    self.svLog(`=> appendChild(${node})`);
    if (self.svExtra.tail) {
        node.svPrevSibling = self.svExtra.tail;
        self.svExtra.tail.svNextSibling = node;
    }
    self.svExtra.tail = node;
    node.parentNode = self;
    node.svMounted = true;
}

export function removeChild(self: any, node: INode) {
    self.svLog(`=> removeChild(${node})`);
    const svPrevSibling = node.svPrevSibling;
    const svNextSibling = node.svNextSibling;
    if (svPrevSibling) {
        svPrevSibling.svNextSibling = svNextSibling;
    }
    if (svNextSibling) {
        svNextSibling.svPrevSibling = svPrevSibling;
    } else if (self.svExtra.tail === node) {
        self.svExtra.tail = svPrevSibling;
    }
    node.svNextSibling = null;
    node.svPrevSibling = null;
    node.parentNode = null;
    node.svMounted = false;
    node.onUnmount?.();
}

export const isWidget = (cls: any): cls is IWidget =>
    cls && !!cls.isGtkWidget && !!cls.isGObject;
export const isNode = (cls: any): cls is Node => cls && cls instanceof Node;
export const isText = (cls: any): cls is Text => cls && cls instanceof Text;
export const isChild = (cls: any): cls is IChild =>
    typeof (cls as any).svName === "string";
export const isComment = (cls: any): cls is Comment =>
    cls && cls instanceof Comment;
export const isStyle = (cls: any): cls is Style => cls && cls instanceof Style;

export const getNextWidget = (node: INode): IWidget | null => {
    let next = node.svNextSibling;
    while (next) {
        if (isWidget(next) && next.svMounted) {
            break;
        }
        next = next.svNextSibling;
    }
    return isWidget(next) ? next : null;
};

export const getPrevWidget = (node: INode): INode | null => {
    let prev = node.svPrevSibling;
    while (prev) {
        if (isWidget(prev) && prev.svMounted) {
            break;
        }
        prev = prev.svPrevSibling;
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
    } else if (typeof x?.svTagName === "string") {
        let res = `<${x.tagName}`;
        if (x?.$?.hasOwnProperty?.("id")) {
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

export const createToString = (x: any) => () => str(x);

export const createLogger =
    (_: any) =>
    (...msg: any) => {
        console.log(`<${_}>`, ...msg);
    };
