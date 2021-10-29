const fs = require("fs");
const path = require("path");
const esbuild = require("esbuild");
const esbuildSvelte = require("esbuild-svelte");
const sveltePreprocess = require("svelte-preprocess");

const DISTDIR = "./dist/";
const OUTFILE = path.join(DISTDIR, "index.js");
const IS_PROD = process.env.NODE_ENV === "production";
const IS_WATCH = process.env.NODE_ENV === "watch";

if (typeof process.env.CLEAN_DIST === "string") {
    fs.rmSync(DISTDIR, { recursive: true, force: true });
}

if (!fs.existsSync(DISTDIR)) {
    fs.mkdirSync(DISTDIR);
}
console.log(`[${new Date().toLocaleTimeString()}] Building...`);

esbuild
    .build({
        entryPoints: ["app/index.ts"],
        outfile: OUTFILE,
        format: "esm",
        target: "es2020",
        bundle: true,
        minify: IS_PROD,
        splitting: false,
        sourcemap: false,
        // external: ["svelte", "svelte/*"],
        mainFields: ["svelte", "module"],
        plugins: [
            esbuildSvelte({
                preprocess: sveltePreprocess({
                    typescript: true,
                    sourceMap: false,
                }),
                compileOptions: {
                    dev: true,
                    accessors: true,
                    namespace: "foreign",
                    css: true,
                    sourcemap: false,
                    enableSourcemap: { js: true, css: false },
                },
                // sourceMap: false,
            }),
        ],
        watch: IS_WATCH
            ? {
                  onRebuild: () => {
                      console.log(
                          `[${new Date().toLocaleTimeString()}] Rebuild finished`,
                      );
                  },
              }
            : undefined,
    })
    .catch(() => process.exit(1));
