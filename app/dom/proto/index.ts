import { initBaseNode, initSingleton, initWidget } from "./base";
import * as w from "./widgets";
import { ListViewProxy } from "./proxies";

let initialized = false;
function initializeClasses() {
    if (initialized) return;
    /// Basic getters and setters
    initBaseNode("Button", GtkButton);
    // initBaseNode("Label", GtkLabel);
    initBaseNode("Box", GtkBox);
    initBaseNode("Paned", GtkPaned);
    initBaseNode("Scrolled", GtkScrolledWindow);
    initBaseNode("Viewport", GtkViewport);
    initBaseNode("Motion", GtkEventControllerMotion);
    initBaseNode("Keyboard", GtkEventControllerKey);
    initBaseNode("Focus", GtkEventControllerFocus);
    initBaseNode("SignalListItemFactory", GtkSignalListItemFactory);
    initBaseNode("MultiSelection", GtkMultiSelection);
    initBaseNode("SingleSelection", GtkSingleSelection);
    initBaseNode("NoSelection", GtkNoSelection);
    initBaseNode("AppWindow", GtkApplicationWindow);
    initBaseNode("GtkWindow", GtkWindow);
    initBaseNode("ListView", GtkListView);
    initBaseNode("CallbackListModel", CallbackListModel);
    initBaseNode("ListItemValue", ListItemValue);

    /// Widgets with only one child
    initSingleton(GtkScrolledWindow);
    initSingleton(GtkViewport);
    w.initAppWindow();
    w.initGtkWindow();
    w.initListView();
    w.initSelection(GtkMultiSelection);
    w.initSelection(GtkSingleSelection);
    w.initSelection(GtkNoSelection);
    w.initPaned(GtkPaned);
    w.initBox(GtkBox);

    initWidget(GtkButton, {
        controller: (self, node, remove) => {
            remove ? self.removeController(node) : self.addController(node);
        },
    });
    // initWidget(Label, {
    //     controller: (self, node, remove) => {
    //         remove ? self.removeController(node) : self.addController(node);
    //     },
    // });

    initialized = true;
}

if (!application.isInitialized) {
    initializeClasses();
    application.isInitialized = true;
}

export const ELEMENTS: { [name: string]: () => any } = {
    button: () => new GtkButton(),
    // label: () => new Label(),
    hBox: () => new GtkBox("horizontal"),
    vBox: () => new GtkBox("vertical"),
    hPaned: () => new GtkPaned("horizontal"),
    vPaned: () => new GtkPaned("vertical"),
    scrolled: () => new GtkScrolledWindow(),
    viewport: () => new GtkViewport(),
    motion: () => new GtkEventControllerMotion(),
    keyboard: () => new GtkEventControllerKey(),
    focus: () => new GtkEventControllerFocus(),
    signalListItemFactory: () => new GtkSignalListItemFactory(),
    multiSelection: () => new GtkMultiSelection(),
    singleSelection: () => new GtkSingleSelection(),
    noSelection: () => new GtkNoSelection(),
    appWindow: () => new GtkApplicationWindow(),
    gtkWindow: () => new GtkWindow(),
    listView: () => new GtkListView(),
    listViewProxy: () => new ListViewProxy(),
};
