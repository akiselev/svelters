/// <reference path="./dom/domtypes.d.ts" />

import "./dom/document.ts";
import App from "./App.svelte";

const shutdown = () => {
    document?.cleanup();
};

const start = (reloaded: boolean) => {
    let state = null;
    if (reloaded) {
        state = document?.app?.$capture_state();
        try {
            document?.cleanup?.();
        } catch (err) {
            console.error("ERROR ON RELOAD: ", err);
            console.error(err.stack);
        }
    }
    document.app = new App({ target: document.root });
    if (state) {
        document.app.$inject_state(state);
    }
    document.cleanup = () => {
        application.removeEventListener("shutdown", shutdown);
        document?.app?.$destroy?.();
    };
};

export default start;
