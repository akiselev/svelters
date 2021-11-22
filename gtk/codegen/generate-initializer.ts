import { Generator } from "./generator";
import * as constants from "./constants";

// fs.writeFileSync("gtk/types/generated.rs", generateInitialize(), { encoding: "utf8" });
export const generateInitializer = async (
    packages: string[],
): Promise<string> => {
    const gen = new Generator(packages);
    await gen.initialize();

    let output = "use crate::Registry;\nuse glib::StaticType;\n\n";
    let types = [];
    Object.keys(gen.types).forEach((cls) => {
        if (gen.implements(cls, "GObject.Object") && gen.canGenerate(cls)) {
            const name = cls.split(".", 2)[1];
            const g = `${gen.getRustPrefix(cls)}${name}`;
            // output += "\n" + gen.generateClass(cls);
            types.push(`ctx.initialize(${g}::static_type())?;`);
        }
    });

    output += `
pub fn initialize_types<'js>(ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<()> {
${types.map((ty) => `    ${ty}`).join("\n")}
    Ok(())
}
`;

    return output;
};
