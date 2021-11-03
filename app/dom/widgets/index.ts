import { Widget } from "./widget";
import { Singleton } from "./singleton";
import { LayoutWidget } from "./layout";
import { IContainer, IWidget } from "../nodes";

const Layout = <T>(tagName: string, el: T): IWidget & IContainer => {
    return LayoutWidget(tagName, el as unknown as Box, {
        onInsertWidgetAfter: ($, node, anchor) => {
            $.insertChildAfter(node, anchor);
        },
        onAppendWidget: ($, node) => {
            $.append(node);
        },
        onRemoveWidget: ($, node) => {
            $.remove(node);
        },
    });
};

const appWindow = () => {
    return Singleton("appWindow", new ApplicationWindow(), {
        children: {
            titlebar: ($, node) => {
                $.titlebar = node;
            },
        },
    });
};

const gtkWindow = () => {
    return Singleton("gtkWindow", new Window(), {
        children: {
            titlebar: ($, node) => {
                $.titlebar = node;
            },
        },
    });
};

export const ELEMENTS: { [name: string]: () => any } = {
    button: () => Widget("button", new Button()),
    hBox: () => Layout("hBox", new HorizontalBox()),
    vBox: () => Layout("vBox", new VerticalBox()),
    appWindow,
    gtkWindow,
};
