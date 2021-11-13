/**
 * Default values, parse the config file and handle CLI flags
 */

import { cosmiconfig, Options as ConfigSearchOptions } from "cosmiconfig";
import Path from "path";
import OS from "os";
import { Utils } from "./utils";
import {
    Environment,
    BuildType,
    UserConfig,
    ConfigFlags,
    UserConfigLoadResult,
    GenerateConfig,
} from "./types";
import { promises as fs } from "fs";
import { Logger } from "./logger";

export class Config {
    static appName = "ts-for-gir";

    static configFilePath = Path.join(process.cwd(), ".ts-for-girrc.js");

    /**
     * Default cli flag and argument values
     */
    static defaults = {
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
    };

    /**
     * Overwrites values in the user config
     * @param configsToAdd
     */
    public static async addToConfig(
        configsToAdd: Partial<UserConfig>,
    ): Promise<void> {
        const userConfig = await this.loadConfigFile();
        const path = userConfig?.filepath || this.configFilePath;
        const configToStore = {};
        Utils.merge(configToStore, userConfig?.config || {}, configsToAdd);
        const fileExtension = Path.extname(path);
        let writeConfigString = "";
        switch (fileExtension) {
            case ".js":
                writeConfigString = `module.exports = ${JSON.stringify(
                    configToStore,
                    null,
                    4,
                )}`;
                break;
            case ".json":
                writeConfigString = `${JSON.stringify(configToStore, null, 4)}`;
                break;
            default:
                Logger.error(
                    "Only configs with the extension .js and .json are currently supported. Do nothing",
                );
                break;
        }
        if (writeConfigString && path) {
            return fs.writeFile(path, writeConfigString);
        }
    }

    /**
     * The user can create a `.ts-for-girrc` file for his default configs,
     * this method load this config file an returns the user configuration
     * @param configName If the user uses a custom config file name
     */
    private static async loadConfigFile(
        configName?: string,
    ): Promise<UserConfigLoadResult | null> {
        const configSearchOptions: ConfigSearchOptions = {};
        if (configName) {
            configSearchOptions.searchPlaces = [configName];
        }
        const userConfig: UserConfigLoadResult | null = await cosmiconfig(
            Config.appName,
            configSearchOptions,
        ).search();
        if (userConfig?.filepath) {
            Config.configFilePath = userConfig?.filepath;
        }
        return userConfig;
    }

    public static getGenerateConfig(
        config: UserConfig,
        environment: Environment = "gjs",
    ): GenerateConfig {
        const defaultBuildType = environment === "gjs" ? "lib" : "types";
        const generateConfig: GenerateConfig = {
            environment: environment,
            girDirectories: config.girDirectories,
            outdir: config.outdir,
            pretty: config.pretty,
            verbose: config.verbose,
            buildType: config.buildType || defaultBuildType,
            exportDefault: config.exportDefault,
        };
        return generateConfig;
    }

    /**
     * Loads the values of the config file and concatenate them with passed cli flags / arguments.
     * The values from config file  are prefered if the cli flag value is the default (and so not set / overwritten)
     * @param flags
     * @param modules
     */
    public static async load(
        flags: ConfigFlags,
        modules: string[],
    ): Promise<UserConfig> {
        const configFile = await this.loadConfigFile(flags.configName);
        const config: UserConfig = {
            environments: flags.environments as Environment[],
            buildType: flags.buildType as BuildType | undefined,
            verbose: flags.verbose,
            ignoreConflicts: flags.ignoreConflicts,
            pretty: flags.pretty,
            print: flags.print,
            outdir: flags.outdir,
            girDirectories: flags.girDirectories,
            ignore: flags.ignore,
            modules,
            exportDefault: flags.exportDefault,
        };

        if (configFile) {
            if (
                Utils.isEqual(
                    config.environments,
                    Config.defaults.environments,
                ) &&
                configFile.config.environments
            ) {
                config.environments = configFile.config.environments;
            }
            if (configFile.config.buildType) {
                config.buildType = configFile.config.buildType;
            }
            if (typeof configFile.config.verbose === "boolean") {
                config.verbose = configFile.config.verbose;
            }
            if (typeof configFile.config.pretty === "boolean") {
                config.pretty = configFile.config.pretty;
            }
            if (typeof configFile.config.print === "boolean") {
                config.print = configFile.config.print;
            }
            if (configFile.config.outdir) {
                config.outdir = config.print ? null : configFile.config.outdir;
            }
            if (configFile.config.girDirectories) {
                config.girDirectories = configFile.config.girDirectories;
            }
            if (
                (!config.ignore ||
                    config.ignore.length <= 0 ||
                    Utils.isEqual(config.ignore, Config.defaults.ignore)) &&
                configFile.config.ignore
            ) {
                config.ignore = configFile.config.ignore;
            }
            if (
                (config.modules.length <= 0 ||
                    Utils.isEqual(config.modules, Config.defaults.modules)) &&
                configFile.config.modules
            ) {
                config.modules = configFile.config.modules;
            }
            if (typeof configFile.config.exportDefault === "boolean") {
                config.exportDefault = configFile.config.exportDefault;
            }
        }
        return config;
    }
}
