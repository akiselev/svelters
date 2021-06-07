import path from "path";
import { loadBinding } from "@node-rs/helper";

import { SvelteQt } from "./types";

const _path = path.resolve(path.join(__dirname, "../"));
console.log("Loading rust library....", _path);

/**
 * __dirname means load native addon from current dir
 * 'svelte-qt' means native addon name is `svelte-qt`
 * the first arguments was decided by `napi.name` field in `package.json`
 * the second arguments was decided by `name` field in `package.json`
 * loadBinding helper will load `svelte-qt.[PLATFORM].node` from `__dirname` first
 * If failed to load addon, it will fallback to load from `svelte-qt-[PLATFORM]`
 */

const { QMainWindow, QApplication, QPushButton } = loadBinding(
    _path,
    "svelte-qt",
    "svelte-qt",
) as SvelteQt;

export { QMainWindow, QApplication, QPushButton };
