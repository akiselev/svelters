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
        createElement(tagName: string): nodes.Node;
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
        addClass(className: string): void;
        removeClass(className: string): void;
        setAttribute(name: string, value: any): void;
        getAttribute(name: string): any;
        removeAttribute(name: string): void;
        addEventListener(type: string, listener: (event: any) => void): void;
        removeEventListener(type: string, listener: (event: any) => void): void;
    }

    class ApplicationWindow extends GtkWidget {
        new(): ApplicationWindow;
    }
    class Window extends GtkWidget {
        new(): Window;
    }
    class GtkBox extends GtkWidget {
        insertChildAfter(child: any, anchor: any): void;
        append(child: any): void;
        prepend(child: any): void;
        remove(child: any): void;
    }
    class HorizontalBox extends GtkBox {
        new(): HorizontalBox;
    }
    class VerticalBox extends GtkBox {
        new(): VerticalBox;
    }
    class Button extends GtkWidget {
        new(): Button;
    }
    class CssProvider extends GtkWidget {
        new(): CssProvider;
    }
}
