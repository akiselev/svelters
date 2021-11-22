const fs = require("fs");
const path = require("path");
const esbuild = require("esbuild");
const esbuildSvelte = require("./dist/esbuild-svelte");
const sveltePreprocess = require("svelte-preprocess");

const DISTDIR = "./dist/codegen/";
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
        entryPoints: ["gtk/codegen/index.ts"],
        // outdir: DISTDIR,
        outfile: OUTFILE,
        format: "cjs",
        target: ["es2020"],
        platform: "node",
        external: ["espree"],
        bundle: true,
        minify: false,
        sourcemap: true,
        plugins: [],
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
    .catch((err) => {
        console.error(err);
        process.exit(1);
    });
