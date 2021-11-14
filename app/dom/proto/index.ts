import { initBaseNode, initSingleton, initWidget } from "./base";
import * as w from "./widgets";

let initialized = false;
function initializeClasses() {
    if (initialized) return;
    /// Basic getters and setters
    initBaseNode("Button", Button);
    // initBaseNode("Label", Label);
    initBaseNode("Box", Box);
    // initBaseNode("Paned", Paned);
    // initBaseNode("scrolled", ScrolledWindow);
    // initBaseNode("viewport", Viewport);
    // initBaseNode("motion", EventControllerMotion);
    // initBaseNode("keyboard", EventControllerKey);
    // initBaseNode("focus", EventControllerFocus);
    // initBaseNode("signalListItemFactory", SignalListItemFactory);
    // initBaseNode("multiSelection", MultiSelection);
    // initBaseNode("singleSelection", SingleSelection);
    // initBaseNode("noSelection", NoSelection);
    initBaseNode("AppWindow", ApplicationWindow);
    // initBaseNode("gtkWindow", Window);
    // initBaseNode("listView", ListView);

    /// Widgets with only one child
    // initSingleton(ScrolledWindow);
    // initSingleton(Viewport);
    w.initAppWindow();
    // w.initGtkWindow();
    // w.initListView();
    // w.initSelection(MultiSelection);
    // w.initSelection(SingleSelection);
    // w.initSelection(NoSelection);
    // w.initPaned(HorizontalPaned);
    // w.initPaned(VerticalPaned);
    // w.initBox(HorizontalBox);
    w.initBox(Box);

    initWidget(Button, {
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
    button: () => new Button(),
    // label: () => new Label(),
    hBox: () => new Box("horizontal"),
    vBox: () => new Box("vertical"),
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
    appWindow: () => new ApplicationWindow(),
    // gtkWindow: () => new Window(),
    // listView: () => new ListView(),
};
