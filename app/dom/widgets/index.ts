import { Widget } from "./widget";
import { Singleton } from "./singleton";
import { LayoutWidget } from "./layout";
import { IWidget } from "../nodes";

const Layout = <T>(tagName: string, el: T): IWidget => {
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

const Paned = <T extends Paned>(tagName: string, el: T): IWidget => {
    const start = ($: any, node: any, remove: any) => {
        $.startChild = remove ? null : node;
    };
    const end = ($: any, node: any, remove: any) => {
        $.endChild = remove ? null : node;
    };
    return Widget(tagName, el, {
        start,
        end,
        left: start,
        right: end,
        top: start,
        bottom: end,
    });
};

const appWindow = () => {
    return Singleton("appWindow", new ApplicationWindow(), {
        children: {
            titlebar: ($, node, remove) => {
                $.titlebar = remove ? null : node;
            },
            controller: ($, node, remove) => {
                remove ? $.removeController(node.$) : $.addController(node.$);
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
            controller: ($, node, remove) => {
                remove ? $.removeController(node.$) : $.addController(node.$);
            },
        },
    });
};

const listView = () => {
    return Widget("listView", new ListView(), {
        model: (self, child, remove) => {
            self.model = remove ? null : child;
        },
        factory: (self, child, remove) => {
            self.factory = remove ? null : child;
        },
    });
};

const Selection = <T>(tagName: string, el: { model: any }): IWidget => {
    return Widget(tagName, el, {
        model: (self, child, remove) => {
            self.model = remove ? null : child;
        },
    });
};

export const ELEMENTS: { [name: string]: () => any } = {
    button: () => Widget("button", new Button()),
    label: () => Widget("label", new Label()),
    hBox: () => Layout("hBox", new HorizontalBox()),
    vBox: () => Layout("vBox", new VerticalBox()),
    hPaned: () => Paned("hPaned", new HorizontalPaned()),
    vPaned: () => Paned("vPaned", new VerticalPaned()),
    scrolled: () => Singleton("scrolled", new ScrolledWindow()),
    viewport: () => Singleton("viewport", new Viewport()),
    motion: () => Widget("motion", new EventControllerMotion()),
    keyboard: () => Widget("keyboard", new EventControllerKey()),
    focus: () => Widget("focus", new EventControllerFocus()),
    signalListItemFactory: () =>
        Widget("signalListItemFactory", new SignalListItemFactory()),
    multiSelection: () => Selection("multiSelection", new MultiSelection()),
    singleSelection: () => Selection("singleSelection", new SingleSelection()),
    noSelection: () => Selection("noSelection", new NoSelection()),
    appWindow,
    gtkWindow,
    listView,
};
