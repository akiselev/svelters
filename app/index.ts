/// <reference path="./dom/domtypes.d.ts" />

import "./dom/document.ts";
import App from "./App.svelte";

const start = (reloaded: boolean) => {
    if (reloaded) {
        try {
            document?.app?.$destroy?.();
        } catch (err) {
            console.error("ERROR ON RELOAD: ", err);
            console.error(err.stack);
        }
    }
    document.app = new App({ target: document.root });
};

export default start;
