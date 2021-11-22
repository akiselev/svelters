import OS from "os";

import * as constants from "./constants";
import { GirConstruct, InheritanceTable, ParsedGir } from "./types";
import { ModuleLoader } from "./module-loader";
import { GirModule } from "./gir-module";

const GIR_DIRECTORY =
    OS.platform() === "darwin"
        ? "/usr/local/share/gir-1.0"
        : "/usr/share/gir-1.0";

export class Generator {
    types: { [name: string]: GirConstruct | null } = {};
    cTypes: { [name: string]: GirConstruct | null };
    modules: { [name: string]: GirModule } = {};
    inheritance: InheritanceTable = {};

    loader: ModuleLoader = new ModuleLoader(constants.DEFAULT_CONFIG as any);

    constructor(private packages: string[]) {}

    initialize = async () => {
        const { grouped } = await this.loader.getModules(this.packages);
        Object.values(grouped).forEach(({ modules }) =>
            modules.forEach(({ module }) => {
                console.log(module.name.toLowerCase());
                this.modules[module.name.toLowerCase()] = module;
                module.loadTypes(this.types);
            }),
        );
        Object.values(grouped).forEach(({ modules }) =>
            modules.forEach(({ module }) =>
                module.loadInheritance(this.inheritance),
            ),
        );
        this.cTypes = Object.fromEntries(
            Object.entries(this.types)
                .filter(([key, ty]) => typeof ty.$["c:type"] === "string")
                .map(([key, ty]) => {
                    if (this.inherits(key, "GObject.Object")) {
                        return [`ty.$["c:type"]*`, ty];
                    }
                    return [ty.$["c:type"], ty];
                }),
        );
    };

    implements = (cls: string, target: string) => {
        return cls === target || this.getParents(cls).indexOf(target) > -1;
    };

    inherits = (cls: string, target: string) => {
        return this.getParents(cls).indexOf(target) > -1;
    };

    getRustPrefix = (cls: string) => {
        if (cls.startsWith("Gtk.")) {
            return "gtk4::";
        } else if (cls.startsWith("Gio.")) {
            return "gio::";
        } else if (cls.startsWith("Gdk.")) {
            return "gdk4::";
        } else if (cls.startsWith("GLib.")) {
            return "gtk4::glib::";
        } else if (cls.startsWith("GObject.")) {
            return "gtk4::glib::";
        }
        throw new Error(`Could not find lib for ${cls}`);
    };

    canGenerate = (cls: string) => {
        const name = cls.split(".", 2)[1];
        return (
            (cls.startsWith("Gtk.") ||
                cls.startsWith("GLib.") ||
                cls.startsWith("Gdk.")) &&
            constants.IGNORE_TYPES.indexOf(name) == -1
        );
        // try {
        //     this.getRustPrefix(cls);
        //     return true;
        // } catch {
        //     return false;
        // }
    };

    getParents = (cls: string): string[] => {
        let out = [];
        for (const parent of this.inheritance[cls] || []) {
            if (out.indexOf(parent) === -1) {
                out.push(parent);
                const parents = this.getParents(parent);
                for (const parent of parents) {
                    if (out.indexOf(parent) === -1) {
                        out.push(parent);
                    }
                }
            }
        }
        return out;
    };

    generateClass = (cls: string): string => {
        const name = cls.split(".", 2)[1];
        const lib = this.getRustPrefix(cls);
        const ty = `${lib}${name}`;
        const inherits = this.getParents(cls)
            .filter((cls) => cls !== "GObject.Object")
            .map(
                (ty) =>
                    `crate::newtype::Gtk<${this.getRustPrefix(ty)}${
                        ty.split(".", 2)[1]
                    }>`,
            );
        return `
pub type ${name} = crate::newtype::Gtk<${lib}${name}>;

crate::common::interface!(
    ${name} as ${ty} |ty| {

    }
);

crate::common::class_def!(
    ${name} as "${name}" |ty| {
        crate::core::from_gtype::define::<${ty}, _>(&ty)?;
        ty.implements::<${name}>()?;
        ty.implements::<crate::Object>()?;
${inherits.map((ty) => `        ty.implements::<${ty}>()?;`).join("\n")}
        ty.register_as::<${ty}>()?;
    }
);
`;
    };
}
