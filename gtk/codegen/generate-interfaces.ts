import { Generator } from "./generator";
import * as constants from "./constants";
import { GirClass } from "./types/gir-class";
import { GirFunction } from "./types/gir-function";
import { camelCase } from "change-case";

interface Options {
    skipProperties?: boolean;
}

export const generateInterfaces = async (
    pkg: string,
    opts: Options = {},
): Promise<string> => {
    const gen = new Generator(constants.GTK_PACKAGE);
    await gen.initialize();

    let output = "use crate::Registry;\nuse glib::StaticType;\n\n";
    let errors = "";
    let types = [];
    for (let key in gen.types) {
        if (!gen.implements(key, "GObject.Object")) continue;
        let ty = gen.types[key] as GirClass;
        const [ns, name] = key.split(".", 2);
        /// Ignoring anything not in our namespace
        if (ns.toLowerCase() !== pkg.toLowerCase()) continue;
        const path = `${gen.getRustPrefix(key)}${name}`;
        output += startInterface(name, path);

        let count = 0;
        ty.method?.forEach((method) => {
            console.log(JSON.stringify(method, strip, 2));
            try {
                output += generateMethod(method, opts);
            } catch (err) {
                errors += ` ==== ERROR in ${name}::${
                    method.$["c:identifier"] || method.$.name
                } ====\n`;
                errors += ` ${err}`;
                if (err.stack) {
                    errors += ` ${err.stack}\n`;
                }
                errors += ` ===============\n`;
            }
        });

        output += endInterface();
    }

    output +=
        "\n\n" +
        errors
            .split("\n")
            .map((line) => `//   ${line}`)
            .join("\n");

    return output;
};

const startInterface = (name: string, path: string) => `
pub macro ${name} () {
    crate::interface!(
        ${path} {
`;

const endInterface = () => `
    });
}`;

const strip = (name, value) => {
    if (
        ["ns", "alias", "symTable", "repo", "namespace", "_module"].indexOf(
            name,
        ) > -1
    ) {
        return;
    }
    return value;
};

const generateMethod = (method: GirFunction, opts: Options): string => {
    let output = "";
    if (method.$["glib:set-property"] && opts.skipProperties) return output;
    let data = methodData(method, opts);

    // output += startMethod();

    return output;
};

const methodData = (method: GirFunction, opts: Options) => {
    let fn_name = camelCase(method.$.name);
    let c_name = method.$["c:identifier"];
    if (typeof c_name !== "string") throw new Error("No c:identifier");
    return { fn_name, c_name };
};
