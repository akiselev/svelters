import { Generator } from "./generator";
import * as constants from "./constants";
import { GirClass } from "./types/gir-class";
import { GirFunction } from "./types/gir-function";
import { camelCase } from "change-case";
import { GirVariable } from "./types/gir-variable";
import { getGirVariableType, stripTypeJSON } from "./utils";

interface Options {
    skipProperties?: boolean;
}

export const listTypes = async (
    pkg: string,
    opts: Options = {},
): Promise<string[]> => {
    const gen = new Generator(constants.GTK_PACKAGE);
    await gen.initialize();

    let types = new Set<string>();
    for (let key in gen.types) {
        try {
            if (!gen.implements(key, "GObject.Object")) continue;
            let ty = gen.types[key] as GirClass;
            const [ns, name] = key.split(".", 2);
            /// Ignoring anything not in our namespace
            if (ns.toLowerCase() !== pkg.toLowerCase()) continue;
            const path = `${gen.getRustPrefix(key)}${name}`;

            ty.method?.forEach((method) => {
                addTypes(types, method, opts);
            });
        } catch (err) {
            console.error(`Error in ${key}`);
            throw err;
        }
    }
    return [...types.values()];
};

const addTypes = (types: Set<string>, method: GirFunction, opts: Options) => {
    if (method.$["glib:set-property"] && opts.skipProperties) return;
    let data = methodData(method, opts);

    for (const param of method.parameters || []) {
        for (const p of param["instance-parameter"] || []) {
            const ty = getGirVariableType(p, method);
            ty && types.add(ty);
        }
        for (const p of param["parameter"] || []) {
            const ty = getGirVariableType(p, method);
            ty && types.add(ty);
        }
    }
    if (method["return-value"] && method["return-value"].length != 1) {
        throw new Error(
            `Found more than one return value in ${
                method.$.name
            }: ${JSON.stringify(method, stripTypeJSON(strip), 2)}`,
        );
    }
    const ty = getGirVariableType(method["return-value"][0], method);
    ty && types.add(ty);
};

const methodData = (method: GirFunction, opts: Options) => {
    let fn_name = camelCase(method.$.name);
    let c_name = method.$["c:identifier"];
    if (typeof c_name !== "string") throw new Error("No c:identifier");
    return { fn_name, c_name };
};

const strip = ["ns", "alias", "symTable", "repo", "namespace", "_module"];
