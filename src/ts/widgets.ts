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

export class PushButton extends QPushButton {
    private _index = 0;
    private _events: Map<string, ((arg: any) => void)[]> = new Map();
    private _map: Map<any, number> = new Map();
    addEventListener = (event: string, listener: (event: any) => void) => {
        super.addEventListener(event, listener);
    };

    removeEventListener = (name: string, fn: (val: any) => void) => void {};
}

export { QMainWindow, QApplication, QPushButton };
