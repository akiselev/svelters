import OS from "os";
import fs from "fs";
import * as xml2js from "xml2js";

import { Utils } from "./utils";
import { GirModule } from "./gir-module";
import { ParsedGir } from "./types";

const GIR_DIRECTORY =
    OS.platform() === "darwin"
        ? "/usr/local/share/gir-1.0"
        : "/usr/share/gir-1.0";

const generate = async (packageName: string, config: any = {}) => {
    const file = Utils.findFileInDirs([GIR_DIRECTORY], packageName + ".gir");
    if (!file.exists || file.path === null) {
        return null;
    }
    console.log(`Parsing ${file.path}...`);
    const fileContents = fs.readFileSync(file.path, "utf8");
    const result = (await xml2js.parseStringPromise(fileContents)) as ParsedGir;
    const gi = new GirModule(result, {
        environments: ["gjs", "node"],
        pretty: false,
        print: false,
        outdir: "@types",
        girDirectories: [
            OS.platform() === "darwin"
                ? "/usr/local/share/gir-1.0"
                : "/usr/share/gir-1.0",
        ],
        modules: ["*"],
        ignore: [],
        verbose: true,
        ignoreConflicts: false,
        exportDefault: false,
    });

    const types = {};
    gi.loadTypes(types);
    console.log(types["Gtk.Application"]);
    fs.writeFileSync(
        "GtkApplication.json",
        JSON.stringify(types["Gtk.Application"], stripJson, 4),
        { encoding: "utf8" },
    );
};

generate("Gtk-4.0");

const STRIP = ["_module", "_ns"];
const stripJson = (key: string, value: any): any => {
    if (STRIP.indexOf(key) > -1) {
        return undefined;
    }
    return value;
};
