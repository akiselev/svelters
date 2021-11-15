import { initBaseNode, initSingleton, initWidget } from "./base";
import * as w from "./widgets";

let initialized = false;
function initializeClasses() {
    if (initialized) return;
    /// Basic getters and setters
    initBaseNode("Button", GtkButton);
    // initBaseNode("Label", GtkLabel);
    initBaseNode("Box", GtkBox);
    // initBaseNode("Paned", GtkPaned);
    // initBaseNode("scrolled", GtkScrolledWindow);
    // initBaseNode("viewport", GtkViewport);
    // initBaseNode("motion", GtkEventControllerMotion);
    // initBaseNode("keyboard", GtkEventControllerKey);
    // initBaseNode("focus", GtkEventControllerFocus);
    // initBaseNode("signalListItemFactory", GtkSignalListItemFactory);
    // initBaseNode("multiSelection", GtkMultiSelection);
    // initBaseNode("singleSelection", GtkSingleSelection);
    // initBaseNode("noSelection", GtkNoSelection);
    initBaseNode("AppWindow", GtkApplicationWindow);
    // initBaseNode("gtkWindow", GtkWindow);
    // initBaseNode("listView", GtkListView);

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

initializeClasses();

export const ELEMENTS: { [name: string]: () => any } = {
    button: () => new GtkButton(),
    // label: () => new Label(),
    hBox: () => new GtkBox("horizontal"),
    vBox: () => new GtkBox("vertical"),
    // hPaned: () => new Paned("horizontal"),
    // vPaned: () => new Paned("vertical"),
    // scrolled: () => new ScrolledWindow(),
    // viewport: () => new Viewport(),
    // motion: () => new EventControllerMotion(),
    // keyboard: () => new EventControllerKey(),
    // focus: () => new EventControllerFocus(),
    // signalListItemFactory: () => new SignalListItemFactory(),
    // multiSelection: () => new MultiSelection(),
    // singleSelection: () => new SingleSelection(),
    // noSelection: () => new NoSelection(),
    appWindow: () => new GtkApplicationWindow(),
    // gtkWindow: () => new Window(),
    // listView: () => new ListView(),
};
