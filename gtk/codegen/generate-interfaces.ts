import { Generator } from "./generator";
import * as constants from "./constants";
import { GirClass } from "./types/gir-class";

export const generateInterfaces = async (pkg: string): Promise<string> => {
    const gen = new Generator(constants.MAIN_PACKAGES);
    await gen.initialize();

    let output = "use crate::Registry;\nuse glib::StaticType;\n\n";
    let types = [];
    for (let key in gen.types) {
        if (!gen.implements(key, "GObject.Object")) continue;
        let ty = gen.types[key] as GirClass;
        const [ns, name] = key.split(".", 2);
        /// Ignoring anything not in our namespace
        if (ns.toLowerCase() !== pkg.toLowerCase()) continue;
        const path = `${gen.getRustPrefix(key)}${name}`;
        output += startInterface(name, path);

        output += endInterface();
    }

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
