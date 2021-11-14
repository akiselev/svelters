import OS from "os";
import fs from "fs";
import * as xml2js from "xml2js";

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

    loader: ModuleLoader = new ModuleLoader(defaultConfig as any);

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

    inherits = (cls: string, target: string) => {
        return this.getParents(cls).indexOf(target) > -1;
    };

    getParents = (cls: string): string[] => {
        let out = [];
        for (const parent of this.inheritance[cls] || []) {
            out = [...out, parent, ...this.getParents(parent)];
        }
        return out;
    };

    generateInterfaces = () => {};
}

const defaultConfig = {
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
