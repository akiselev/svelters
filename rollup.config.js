// rollup.config.js
import svelte from "rollup-plugin-svelte";
import resolve from "@rollup/plugin-node-resolve";
import commonjs from "@rollup/plugin-commonjs";
import typescript from "rollup-plugin-typescript2";
// import native from "rollup-plugin-native";

export default {
    input: "src/ts/index.ts",
    output: {
        file: "dist/index.js",
        format: "commonjs",
    },
    external: ["@node-rs/helper"],
    plugins: [
        svelte(require("./svelte.config.js")),
        resolve({ browser: false }),
        commonjs(),
        typescript(),
        // native({
        //     platformName:
        //         "${dirname}/svelte-qt.${nodePlatform}-${nodeArchitecture}.node",
        //     // "${dirname}/precompiled/${nodePlatform}-${nodeArchitecture}/node.napi.node",

        //     // // Where we want to physically put the extracted .node files
        //     // copyTo: "dist/",

        //     // // Path to the same folder, relative to the output bundle js
        //     // destDir: "../",

        //     // // Use `dlopen` instead of `require`/`import`.
        //     // // This must be set to true if using a different file extension that '.node'
        //     // dlopen: false,

        //     // // Generate sourcemap
        //     // sourcemap: true,
        // }),
    ],
};
