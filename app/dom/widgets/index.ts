import { WidgetCls } from "./widget";
import { Singleton } from "./singleton";
import { LayoutWidget } from "./layout";
import { Constructor, IWidget } from "../nodes";

const Layout = <T extends Constructor<GtkWidget>>(
    tagName: string,
    el: T,
): Constructor<IWidget> => {
    return LayoutWidget(tagName, el, {
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

const Paned = <T extends Constructor<GtkWidget>>(
    tagName: string,
    el: T,
): Constructor<IWidget> => {
    const start = ($: any, node: any, remove: any) => {
        $.startChild = remove ? null : node;
    };
    const end = ($: any, node: any, remove: any) => {
        $.endChild = remove ? null : node;
    };
    return WidgetCls(tagName, el, {
        start,
        end,
        left: start,
        right: end,
        top: start,
        bottom: end,
    });
};

const EL: { [name: string]: Constructor<{}> } = {
    button: WidgetCls("button", Button),
    label: WidgetCls("label", Label),
    hBox: Layout("hBox", HorizontalBox),
    vBox: Layout("vBox", VerticalBox),
    hPaned: Paned("hPaned", HorizontalPaned),
    vPaned: Paned("vPaned", VerticalPaned),
    scrolled: Singleton("scrolled", ScrolledWindow),
    viewport: Singleton("viewport", Viewport),
    motion: WidgetCls("motion", EventControllerMotion),
    keyboard: WidgetCls("keyboard", EventControllerKey),
    focus: WidgetCls("focus", EventControllerFocus),
    gtkWindow: Singleton("gtkWindow", Window, {
        children: {
            titlebar: ($, node) => {
                $.titlebar = node;
            },
            controller: ($, node, remove) => {
                remove ? $.removeController(node) : $.addController(node);
            },
        },
    }),
    appWindow: Singleton("appWindow", ApplicationWindow, {
        children: {
            titlebar: ($, node, remove) => {
                $.titlebar = remove ? null : node;
            },
            controller: ($, node, remove) => {
                remove ? $.removeController(node) : $.addController(node);
            },
        },
    }),
};

export const ELEMENTS: { [name: string]: () => any } = {
    button: () => new EL["button"](),
    label: () => new EL["label"](),
    hBox: () => new EL["hBox"](),
    vBox: () => new EL["vBox"](),
    hPaned: () => new EL["hPaned"](),
    vPaned: () => new EL["vPaned"](),
    scrolled: () => new EL["scrolled"](),
    viewport: () => new EL["viewport"](),
    motion: () => new EL["motion"](),
    keyboard: () => new EL["keyboard"](),
    focus: () => new EL["focus"](),
    appWindow: () => new EL["appWindow"](),
    gtkWindow: () => new EL["gtkWindow"](),
};
