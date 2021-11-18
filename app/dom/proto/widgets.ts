import { initLayout, initSingleton, initWidget } from "./base";
import { IWidget } from "../nodes";

export const initBox = (el: any) => {
    initLayout(el, {
        onInsertWidgetAfter: (self, node, anchor) => {
            self.insertChildAfter(node, anchor);
        },
        onAppendWidget: (self, node) => {
            self.append(node);
        },
        onRemoveWidget: (self, node) => {
            self.remove(node);
        },
    });
};

export const initPaned = (el: any) => {
    const start = (self: any, node: any, remove: any) => {
        self.startChild = remove ? null : node;
    };
    const end = (self: any, node: any, remove: any) => {
        self.endChild = remove ? null : node;
    };
    initWidget(el, {
        start,
        end,
        left: start,
        right: end,
        top: start,
        bottom: end,
    });
};

export const initAppWindow = () => {
    initSingleton(GtkApplicationWindow, {
        titlebar: (self, node, remove) => {
            self.titlebar = remove ? null : node;
        },
        controller: (self, node, remove) => {
            remove ? self.removeController(node) : self.addController(node);
        },
    });
};

export const initGtkWindow = () => {
    initSingleton(GtkWindow, {
        titlebar: (self, node, remove) => {
            self.titlebar = remove ? null : node;
        },
        controller: (self, node, remove) => {
            remove ? self.removeController(node) : self.addController(node);
        },
    });
};

export const initListView = () => {
    Object.defineProperty(ListItemValue.prototype, "value", {
        get() {
            return this.getValue();
        },
    });
    initWidget(GtkListView, {
        model: (self, child, remove) => {
            self.model = remove ? null : child.self;
        },
        factory: (self, child, remove) => {
            self.factory = remove ? null : child.self;
        },
    });
};

export const initSelection = <T>(el: any) => {
    initWidget(el, {
        model: (self, child, remove) => {
            self.model = remove ? null : child.self;
        },
    });
};
