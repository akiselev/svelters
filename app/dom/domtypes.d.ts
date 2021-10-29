/// <reference no-default-lib="true"/>
/// <reference path="../../node_modules/typescript/lib/lib.es5.d.ts" />
/// <reference lib="es5" />
/// <reference lib="es6" />
/// <reference lib="es2015" />
/// <reference lib="es2015.iterable" />
/// <reference lib="es2019" />
/// <reference lib="es2020" />

import * as nodes from "./nodes";

declare global {
    var document: {
        createElement(tagName: string): nodes.NodeBase;
        createElementNS(namespace: string, tagName: string): any;
        createTextNode(data: string): nodes.Text;
        createComment(data: string): nodes.Comment;

        root: nodes.Element;
        app?: any;
    };

    class ApplicationWindow {}
    class Window {}
    class HorizontalBox {}
    class VerticalBox {}
    class Button {}
}
