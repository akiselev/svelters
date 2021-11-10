/// <reference no-default-lib="true"/>
/// <reference path="../../node_modules/typescript/lib/lib.es5.d.ts" />
/// <reference path="../../node_modules/typescript/lib/lib.es2015.d.ts" />
/// <reference path="svelters.d.ts" />
/// <reference lib="es5" />
/// <reference lib="es6" />
/// <reference lib="es2015" />
/// <reference lib="es2015.iterable" />
/// <reference lib="es2019" />
/// <reference lib="es2020" />

import * as nodes from "./nodes";

declare global {
    var document: {
        createElement(tagName: string): nodes.INode;
        createElementNS(namespace: string, tagName: string): any;
        createTextNode(data: string): nodes.Text;
        createComment(data: string): nodes.Comment;
        getRootNode(): typeof document;

        root: nodes.Node;
        app?: any;

        styles: any;
        host: any;
        getElementById(id: string): any | null;
        appendChild(child: any): void;
        removeChild(child: any): void;

        dispatchEvent(event: any): void;
        createEvent(type: string): void;
    };

    class GtkWidget {
        isGObject?: boolean;
        isGtkWidget?: boolean;
        extra: { [name: string]: any };
        cast(...args: any): any;
        addClass(className: string): void;
        removeClass(className: string): void;
        setAttribute(name: string, value: any): void;
        getAttribute(name: string): any;
        addController(controller: any): void;
        removeController(controller: any): void;
        removeAttribute(name: string): void;
        addEventListener(type: string, listener: (event: any) => void): void;
        removeEventListener(type: string, listener: (event: any) => void): void;
    }
}
