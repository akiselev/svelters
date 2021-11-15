import OS from "os";

export const PACKAGES = [
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

export const MAIN_PACKAGES = [
    "Gdk-4.0",
    "GdkPixbuf-2.0",
    "Gio-2.0",
    "GLib-2.0",
    "GObject-2.0",
    "Gtk-4.0",
];

export const DEFAULT_CONFIG = {
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

export const IGNORE_TYPES = [
    // Not available in Rust GTK
    "BuilderCScope",
    "TypeModule",
    "CustomLayout",
    "PasswordEntryBuffer",

    // Manually written
    "Application",
    "CssProvider",
    "Widget",
    "Object",
    "Button",
    "Box",
    "ApplicationWindow",
    "Window",

    // Name collision between GTK and GDK
    "Snapshot",
];
