import { Widget } from "./widget";
import { Singleton } from "./singleton";
import { LayoutWidget } from "./layout";
import { IWidget } from "../nodes";
import { initBaseNode, initSingleton } from "./base";

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
            self.model = remove ? null : child.$;
        },
        factory: (self, child, remove) => {
            self.factory = remove ? null : child.$;
        },
    });
};

const Selection = <T>(tagName: string, el: { model: any }): IWidget => {
    return Widget(tagName, el, {
        model: (self, child, remove) => {
            self.model = remove ? null : child.$;
        },
    });
};

let initialized = false;
function initializeClasses() {
    if (initialized) return;
    /// Basic getters and setters
    initBaseNode("button", Button);
    initBaseNode("label", Label);
    initBaseNode("hBox", HorizontalBox);
    initBaseNode("vBox", VerticalBox);
    initBaseNode("hPaned", HorizontalPaned);
    initBaseNode("vPaned", VerticalPaned);
    initBaseNode("scrolled", ScrolledWindow);
    initBaseNode("viewport", Viewport);
    initBaseNode("motion", EventControllerMotion);
    initBaseNode("keyboard", EventControllerKey);
    initBaseNode("focus", EventControllerFocus);
    initBaseNode("signalListItemFactory", SignalListItemFactory);
    initBaseNode("multiSelection", MultiSelection);
    initBaseNode("singleSelection", SingleSelection);
    initBaseNode("noSelection", NoSelection);
    initBaseNode("appWindow", ApplicationWindow);
    initBaseNode("gtkWindow", Window);
    initBaseNode("listView", ListView);

    /// Widgets with only one child
    initSingleton(ScrolledWindow);
    initSingleton(Viewport);
    initSingleton(ApplicationWindow);
    initSingleton(Window);

    initialized = true;
}

initializeClasses();

export const ELEMENTS: { [name: string]: () => any } = {
    button: () => new Button(),
    label: () => new Label(),
    hBox: () => new HorizontalBox(),
    vBox: () => new VerticalBox(),
    hPaned: () => new HorizontalPaned(),
    vPaned: () => new VerticalPaned(),
    scrolled: () => new ScrolledWindow(),
    viewport: () => new Viewport(),
    motion: () => new EventControllerMotion(),
    keyboard: () => new EventControllerKey(),
    focus: () => new EventControllerFocus(),
    signalListItemFactory: () => new SignalListItemFactory(),
    multiSelection: () => new MultiSelection(),
    singleSelection: () => new SingleSelection(),
    noSelection: () => new NoSelection(),
    appWindow: () => new ApplicationWindow(),
    gtkWindow: () => new Window(),
    listView: () => new ListView(),
};
