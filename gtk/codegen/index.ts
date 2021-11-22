import * as fs from "fs";
import {
    subcommands,
    command,
    run,
    positional,
    string,
    optional,
    number as numberType,
} from "cmd-ts";

import * as constants from "./constants";
import { Generator } from "./generator";
import { generateInitializer } from "./generate-initializer";
import { generateInterfaces } from "./generate-interfaces";

const initializer = command({
    name: "generate type initializers",
    description: "Generates code to register types",
    args: {
        output: positional({
            type: string,
            displayName: "output",
            description: "Output file path",
        }),
    },
    handler: async ({ output }) => {
        let data = await generateInitializer(constants.MAIN_PACKAGES);
        console.log(data);
        fs.writeFileSync(output, data, { encoding: "utf8" });
    },
});

const interfaces = command({
    name: "generate type interfaces",
    description: "Generates macros that register interfaces for types",
    args: {
        library: positional({
            type: string,
            displayName: "library",
            description: "Library name",
        }),
        output: positional({
            type: string,
            displayName: "output",
            description: "Output file path",
        }),
    },
    handler: async ({ library, output }) => {
        const data = await generateInterfaces(library);
        console.log(data);
        fs.writeFileSync(output, data, {
            encoding: "utf8",
        });
    },
});

const generate = subcommands({
    name: "parser-cli",
    description: "Metabase hand written parser CLI",
    cmds: {
        initializer,
        interfaces,
    },
});

const codegen = subcommands({
    name: "codegen",
    description: "Svelters-gtk code generator",
    cmds: {
        generate,
    },
});

// parse arguments
run(codegen, process.argv.slice(2));
