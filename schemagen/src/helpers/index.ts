import * as fs from "fs-jetpack";

import TS from "tree-sitter";
import CPP from "tree-sitter-cpp";
import { SyntaxNode } from "../Parser";

export class Parser {
    headers: { [name: string]: Header } = {};
    parser: TS;

    constructor() {
        this.parser = new TS();
        this.parser.setLanguage(CPP);
    }

    parse = (path: string) => {
        if (this.headers[path]) {
            return this.headers[path];
        } else {
            const header = new Header(this.parser, path);
            this.headers[path] = header;
            return header;
        }
    };
}

export class Header {
    tree: TS.Tree;
    original: string;

    constructor(parser: TS, public path: string) {
        const original = fs.read(path);
        if (!original) throw new Error(`Path ${path} is empty`);
        this.original = original;
        this.tree = parser.parse(original.replace("Q_WIDGETS_EXPORT", ""));
    }

    query = (query: string) => {
        console.log(query);
        let q = new TS.Query(CPP, query);
        return q.matches(this.tree.rootNode);
    };

    query_capture = (query: string) => {
        let q = new TS.Query(CPP, query);
        let output = {};
        for (const out of q.captures(this.tree.rootNode)) {
            if (out?.node?.text) {
                output[out.name];
            }
        }
        return output;
    };
}

export function formatMatches(tree: TS.Tree, matches: TS.QueryMatch[]) {
    return matches.map(({ pattern, captures }) => ({
        pattern,
        captures: formatCaptures(tree, captures),
    }));
}

export function formatCaptures(tree: TS.Tree, captures: TS.QueryCapture[]) {
    return captures.map((c) => {
        const _c = { ...c };
        const node = _c.node;
        delete (_c as any).node;
        _c.text = tree.getText(node);
        return _c;
    });
}

export const crawl = (t: any) => {
    const { text, type, children } = t;
    return {
        text: text?.length > 40 ? `${text.slice(0, 40)}...` : text,
        type,
        children: children?.map(crawl),
    };
};
