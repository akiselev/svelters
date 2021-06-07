const autoPreprocess = require("svelte-preprocess");

module.exports = {
    preprocess: autoPreprocess({
        defaults: { script: "typescript" },
    }),

    // By default, all ".svelte" files are compiled
    extensions: [".svelte"],

    // Emit CSS as "files" for other plugins to process. default is true
    emitCss: false,

    // Warnings are normally passed straight to Rollup. You can
    // optionally handle them here, for example to squelch
    // warnings with a particular code
    onwarn: (warning, handler) => {
        // e.g. don't warn on <marquee> elements, cos they're cool
        if (warning.code === "a11y-distracting-elements") return;

        // let Rollup handle all other warnings normally
        handler(warning);
    },

    // You can pass any of the Svelte compiler options
    compilerOptions: {},
};
