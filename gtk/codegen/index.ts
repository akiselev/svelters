import * as fs from "fs";
import {
    subcommands,
    command,
    run,
    positional,
    string,
    optional,
    oneOf,
    number as numberType,
} from "cmd-ts";

import * as constants from "./constants";
import { Generator } from "./generator";
import { generateInitializer } from "./generate-initializer";
import { generateInterfaces } from "./generate-interfaces";
import { listTypes } from "./list-types";
import { classInfo, className, stripTypeJSON } from "./utils";

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

const list = command({
    name: "list types used in methods",
    description: "Generates macros that register interfaces for types",
    args: {
        library: positional({
            type: string,
            displayName: "library",
            description: "Library name",
        }),
    },
    handler: async ({ library }) => {
        const data = await listTypes(library);
        data.forEach((line) => console.log(line));
    },
});

const dump = command({
    name: "dump gir",
    description: "dump gobject introspection data in JSON format",
    args: {
        library: positional({
            type: string,
            displayName: "library",
            description: "Library name",
        }),
        namespace: positional({
            type: oneOf([
                "bitfield",
                "callback",
                "class",
                "constant",
                "enumeration",
                "function",
                "interface",
                "record",
            ]),
            displayName: "namespace",
            description: "Namespace to dump",
        }),
        output: positional({
            type: string,
            displayName: "output",
            description: "JSON file output path",
        }),
    },
    handler: async ({ library, namespace, output }) => {
        const gen = new Generator(constants.GTK_PACKAGE);
        await gen.initialize();

        const data = gen.modules[library].ns[namespace];
        if (!Array.isArray(data)) throw new Error(`Library "${library}" does not have anything in the "${namespace}" namespace`)
        fs.writeFileSync(output, JSON.stringify(data, stripTypeJSON(["_module"]), 2), {
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
        list,
        dump,
    },
});

// parse arguments
run(codegen, process.argv.slice(2));
