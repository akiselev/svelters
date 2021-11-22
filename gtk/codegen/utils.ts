import lodash from "lodash";
import Path from "path";
import fs from "fs";
import { GirVariable } from "./types";

export class Utils {
    public static splitModuleName(packageName: string): {
        packageName: string;
        name: string;
        version: string;
    } {
        // There are modules that use multiple hyphens like 'GUPnP-DLNA-1.0'
        const splitted = packageName.split("-");
        const version = splitted.splice(-1, 1)[0];
        const name = splitted.join("-");
        return {
            packageName,
            name,
            version,
        };
    }

    /**
     * Checking whether some variable is iterable
     * see https://stackoverflow.com/a/32538867
     * @param obj Variable to check for iterable
     */
    public static isIterable(obj: unknown[]): boolean {
        return obj != null && typeof obj[Symbol.iterator] === "function";
    }

    public static isNumeric(str: string): boolean {
        return !isNaN((str as any) - parseFloat(str));
    }

    public static getFirstChar(str: string): string {
        return str.charAt(0);
    }

    public static getLastChar(str: string): string {
        return str.charAt(str.length - 1);
    }

    public static isFirstCharNumeric(str: string): boolean {
        return Utils.isNumeric(this.getFirstChar(str));
    }

    public static camelCase(str: string): string {
        return str
            .replace(/\s(.)|(\s|-|_|\.)(.)/g, (a) => {
                return a.toUpperCase();
            })
            .replace(/(\s|-|_|\.)/g, "");
    }

    public static lowerCamelCase(str: string): string {
        str = this.camelCase(str);
        str = this.getFirstChar(str).toLowerCase() + str.slice(1);
        return str;
    }

    public static upperCamelCase(str: string): string {
        str = this.camelCase(str);
        str = this.getFirstChar(str).toUpperCase() + str.slice(1);
        return str;
    }

    public static findFileInDirs(
        dirs: string[],
        filename: string,
    ): { path: string | null; exists: boolean } {
        let exists = false;
        for (const dir of dirs) {
            const filePath = Path.join(dir, filename);
            exists = fs.existsSync(filePath);
            if (exists) {
                return {
                    path: filePath,
                    exists,
                };
            }
        }
        return {
            path: null,
            exists,
        };
    }

    /**
     * Union (a ∪ b): create a set that contains the elements of both set a and set b.
     * See https://2ality.com/2015/01/es6-set-operations.html#union
     * @param target
     * @param source
     */
    public static union<T>(target: Set<T> | T[], source: Set<T> | T[]): Set<T> {
        return (target = new Set<T>([...target, ...source]));
    }

    public static isEqual = lodash.isEqual;

    public static map = lodash.map;

    public static find = lodash.find;

    public static merge = lodash.merge;
}

export const classInfo = (name: string) => {
    let split = name.split(".");
    if (split.length !== 2) throw new Error(`Could not split name: ${name}`);
    return {
        lib: split[0].toLowerCase(),
        name: split[1],
    };
};

export const className = (name: string) => {
    if (name.indexOf(".") > -1) {
        return classInfo(name).name;
    }
    return name;
};

export const stripTypeJSON =
    (names: string[] = []) =>
    (name: string, value: any) => {
        const skip = ["ns", "namespace", "alias", "symTable", ...names];
        if (skip.indexOf(name) > -1) return;
        return value;
    };

export const getGirVariableType = (
    p: GirVariable,
    parent: any,
): string | null => {
    const strip = ["ns", "alias", "symTable", "repo", "namespace", "_module"];
    if (p.varargs) {
        return null;
    } else if (p.type) {
        if (p.type.length !== 1) {
            throw new Error(
                `Found ${p.type.length} types for parameter ${
                    p.$.name
                }: ${JSON.stringify(parent, stripTypeJSON(strip), 2)}`,
            );
        }
        return p.type[0].$["c:type"];
    } else if (p.array) {
        if (p.array.length !== 1) {
            throw new Error(
                `Found ${p.type.length} types for parameter ${
                    p.$.name
                }: ${JSON.stringify(parent, stripTypeJSON(strip), 2)}`,
            );
        }
        return p.array[0].$["c:type"];
    } else {
        throw new Error(
            `No type for ${JSON.stringify(
                p,
                stripTypeJSON(strip),
                2,
            )}\n ===== in method ===== \n${JSON.stringify(
                parent,
                stripTypeJSON(strip),
                2,
            )}`,
        );
    }
};
