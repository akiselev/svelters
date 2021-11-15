import OS from "os";
import fs from "fs";
import * as xml2js from "xml2js";

import * as constants from "./constants";
import { GirModule } from "./gir-module";
import { GirConstruct, InheritanceTable, ParsedGir } from "./types";
import { Utils } from "./utils";
import { ModuleLoader } from "./module-loader";

const GIR_DIRECTORY =
    OS.platform() === "darwin"
        ? "/usr/local/share/gir-1.0"
        : "/usr/share/gir-1.0";

export class Generator {
    types: { [name: string]: GirConstruct | null } = {};
    inheritance: InheritanceTable = {};

    loader: ModuleLoader = new ModuleLoader(constants.DEFAULT_CONFIG as any);

    constructor(private packages: string[]) {}

    initialize = async () => {
        const { grouped } = await this.loader.getModules(this.packages);
        Object.values(grouped).forEach(({ modules }) =>
            modules.forEach(({ module }) => module.loadTypes(this.types)),
        );
        Object.values(grouped).forEach(({ modules }) =>
            modules.forEach(({ module }) =>
                module.loadInheritance(this.inheritance),
            ),
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

const gen = new Generator(constants.MAIN_PACKAGES);
gen.initialize().then(() => {
    let output = "use crate::RegisterType;\n\n";
    let types = [];
    Object.keys(gen.types).forEach((cls) => {
        if (gen.implements(cls, "GObject.Object") && gen.canGenerate(cls)) {
            const name = cls.split(".", 2)[1];
            const g = `${gen.getRustPrefix(cls)}${name}`;
            output += "\n" + gen.generateClass(cls);
            types.push(
                `ctx.register_type::<crate::newtype::Gtk<${g}>>("${name}")?;`,
            );
        }
    });

    output += `
fn initialize_objects<'js>(ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<()> {
${types.map((ty) => `    ${ty}`).join("\n")}
    Ok(())
}
`;

    fs.writeFileSync("gtk/types/generated.rs", output, { encoding: "utf8" });
});
