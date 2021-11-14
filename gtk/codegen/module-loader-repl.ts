import OS from "os";

import { ModuleLoader } from "./module-loader";
import { InheritanceTable } from "./types";

const packages = [
    "Adw-1",
    "cairo-1.0",
    "DBus-1.0",
    "DBusGLib-1.0",
    "Gdk-4.0",
    "GdkPixbuf-2.0",
    "GdkPixdata-2.0",
    "GdkWayland-4.0",
    "GdkX11-4.0",
    "Gio-2.0",
    "GLib-2.0",
    "GModule-2.0",
    "GObject-2.0",
    "Graphene-1.0",
    "Gsk-4.0",
    "Gtk-4.0",
    "Pango-1.0",
    "PangoCairo-1.0",
];

const config = {
    environment: "node",
    pretty: false,
    outdir: "@types",
    girDirectories: [
        OS.platform() === "darwin"
            ? "/usr/local/share/gir-1.0"
            : "/usr/share/gir-1.0",
    ],
    verbose: true,
    exportDefault: false,
};

const loader = new ModuleLoader(config as any);
loader.getModules(packages).then(async ({ grouped }) => {
    let inheritance = {};
    let types = {};
    Object.values(grouped).forEach(({ modules }) =>
        modules.forEach(({ module }) => module.loadTypes(types)),
    );
    Object.values(grouped).forEach(({ modules }) =>
        modules.forEach(({ module }) => module.loadInheritance(inheritance)),
    );

    // console.log(inheritance);
    console.log(types["Gtk.Widget"]);
    console.log(
        "Is object: ",
        inherits(inheritance, "Gtk.Button", "GObject.Object"),
    );
    console.log(inheritance["GObject.InitiallyUnowned"]);
});

const inherits = (table: InheritanceTable, cls: string, target: string) => {
    return getParents(table, cls).indexOf(target) > -1;
};

const getParents = (table: InheritanceTable, cls: string): string[] => {
    let out = [];
    for (const parent of table[cls] || []) {
        out = [...out, parent, ...getParents(table, parent)];
    }
    return out;
};
