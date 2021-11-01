/// <reference path="./dom/domtypes.d.ts" />

import "./dom/document.ts";
import App from "./App.svelte";

const start = (reloaded: boolean) => {
    if (reloaded) {
        document.app.$destroy();
    }
    document.app = new App({ target: document.root });
};

export default start;
