# @svelters/dom

DOM shim for stock Svelte 5 (`fragments: "tree"`) that drives a `SveltersHost`. Entry points: `@svelters/dom/install` (side-effect import, installs globals from `globalThis.__svelters_host`) and `createDom`/`installDom`. Mapping rules: `docs/notes/p1-js-lane.md`.
